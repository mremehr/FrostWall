use super::bucket::{RenameBucket, RenameOptions, RenameScheme};
use super::dedupe::{detect_content_duplicates, detect_duplicate_numbered_stems};
use super::display_name;
use crate::utils::is_image_file;
use crate::wallpaper::Wallpaper;
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct PlannedRename {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Default, Clone)]
pub struct RenameWarnings {
    pub duplicate_numbered_stems: Vec<String>,
    pub content_duplicates: Vec<String>,
    pub unreadable_files: Vec<String>,
}

#[derive(Debug, Default, Clone)]
pub struct RenameMigrationReport {
    pub cache_files_updated: usize,
    pub session_paths_updated: usize,
    pub collection_paths_updated: usize,
    pub pairing_paths_updated: usize,
    pub thumbnails_removed: usize,
}

#[derive(Debug, Clone)]
pub struct RenameReport {
    pub total_files: usize,
    pub skipped_already_named: usize,
    pub planned: Vec<PlannedRename>,
    pub warnings: RenameWarnings,
    pub migration: RenameMigrationReport,
}

#[derive(Debug, Clone)]
pub(super) struct WallpaperEntry {
    pub(super) path: PathBuf,
    pub(super) extension: String,
    pub(super) bucket: RenameBucket,
    pub(super) current_name: Option<(RenameBucket, u32)>,
}

pub(super) fn plan_rename_operations(dir: &Path, options: &RenameOptions) -> Result<RenameReport> {
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .with_context(|| format!("Failed to read wallpaper directory: {}", dir.display()))?
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.is_file() && is_image_file(path))
        .collect();
    files.sort();

    let total_files = files.len();
    let mut warnings = RenameWarnings::default();
    let mut entries = Vec::new();

    let analyzed: Vec<_> = files
        .par_iter()
        .map(|path| analyze_entry(path, options.scheme))
        .collect();

    for result in analyzed {
        match result {
            Ok(entry) => entries.push(entry),
            Err((path, error)) => {
                warnings
                    .unreadable_files
                    .push(format!("{}: {}", display_name(&path), error));
            }
        }
    }

    entries.sort_by(|a, b| a.path.cmp(&b.path));
    warnings.duplicate_numbered_stems = detect_duplicate_numbered_stems(&entries);

    if options.warn_content_dupes {
        warnings.content_duplicates = detect_content_duplicates(&entries);
    }

    let mut used_numbers: HashSet<(RenameBucket, u32)> = HashSet::new();
    let mut next_number_by_bucket: HashMap<RenameBucket, u32> = HashMap::new();
    let mut skipped_already_named = 0;
    let mut planned = Vec::new();

    for entry in &entries {
        if let Some((bucket, number)) = entry.current_name {
            if bucket == entry.bucket && !options.compact {
                used_numbers.insert((bucket, number));
                skipped_already_named += 1;
            }
        }
    }

    for entry in entries {
        if let Some((bucket, _number)) = entry.current_name {
            if bucket == entry.bucket && !options.compact {
                continue;
            }
        }

        let number = next_free_number(entry.bucket, &used_numbers, &mut next_number_by_bucket);
        used_numbers.insert((entry.bucket, number));

        let target_name = format!(
            "{}-wallpaper{}.{}",
            entry.bucket.prefix(),
            number,
            entry.extension
        );
        let target_path = entry.path.with_file_name(target_name);
        if target_path != entry.path {
            planned.push(PlannedRename {
                from: entry.path,
                to: target_path,
            });
        }
    }

    Ok(RenameReport {
        total_files,
        skipped_already_named,
        planned,
        warnings,
        migration: RenameMigrationReport::default(),
    })
}

fn analyze_entry(
    path: &Path,
    scheme: RenameScheme,
) -> std::result::Result<WallpaperEntry, (PathBuf, String)> {
    let wallpaper =
        Wallpaper::from_path_fast(path).map_err(|error| (path.to_path_buf(), error.to_string()))?;
    let extension = path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .unwrap_or_else(|| "jpg".to_string());

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();

    Ok(WallpaperEntry {
        path: path.to_path_buf(),
        extension,
        bucket: scheme.bucket_for_aspect(wallpaper.aspect_category),
        current_name: parse_numbered_name(file_name, scheme),
    })
}

fn parse_numbered_name(file_name: &str, scheme: RenameScheme) -> Option<(RenameBucket, u32)> {
    let stem = file_name.rsplit_once('.').map(|(stem, _)| stem)?;

    for bucket in scheme.supported_buckets() {
        let prefix = format!("{}-wallpaper", bucket.prefix());
        let Some(number) = stem.strip_prefix(&prefix) else {
            continue;
        };
        if number.is_empty() || !number.chars().all(|ch| ch.is_ascii_digit()) {
            continue;
        }

        if let Ok(parsed) = number.parse::<u32>() {
            return Some((*bucket, parsed));
        }
    }

    None
}

fn next_free_number(
    bucket: RenameBucket,
    used_numbers: &HashSet<(RenameBucket, u32)>,
    next_number_by_bucket: &mut HashMap<RenameBucket, u32>,
) -> u32 {
    let mut next = next_number_by_bucket.get(&bucket).copied().unwrap_or(1);
    while used_numbers.contains(&(bucket, next)) {
        next += 1;
    }
    next_number_by_bucket.insert(bucket, next + 1);
    next
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};

    fn write_png(path: &Path, width: u32, height: u32) {
        let image = ImageBuffer::from_pixel(width, height, Rgba([32_u8, 64, 96, 255]));
        image.save(path).unwrap();
    }

    #[test]
    fn native_scheme_separates_ultrawide_and_landscape() {
        let dir = tempfile::tempdir().unwrap();
        write_png(&dir.path().join("alpha.png"), 5120, 1440);
        write_png(&dir.path().join("beta.png"), 1920, 1080);
        write_png(&dir.path().join("gamma.png"), 1080, 1920);

        let report = plan_rename_operations(
            dir.path(),
            &RenameOptions {
                dry_run: true,
                compact: false,
                warn_content_dupes: false,
                scheme: RenameScheme::Native,
            },
        )
        .unwrap();

        let targets: Vec<String> = report
            .planned
            .iter()
            .filter_map(|rename| rename.to.file_name().and_then(|name| name.to_str()))
            .map(ToOwned::to_owned)
            .collect();

        assert!(targets.contains(&"ultrawide-wallpaper1.png".to_string()));
        assert!(targets.contains(&"landscape-wallpaper1.png".to_string()));
        assert!(targets.contains(&"portrait-wallpaper1.png".to_string()));
    }

    #[test]
    fn incremental_mode_preserves_existing_numbered_name() {
        let dir = tempfile::tempdir().unwrap();
        write_png(&dir.path().join("landscape-wallpaper4.png"), 1920, 1080);
        write_png(&dir.path().join("fresh.png"), 1920, 1080);

        let report = plan_rename_operations(
            dir.path(),
            &RenameOptions {
                dry_run: true,
                compact: false,
                warn_content_dupes: false,
                scheme: RenameScheme::Native,
            },
        )
        .unwrap();

        assert_eq!(report.skipped_already_named, 1);
        assert_eq!(report.planned.len(), 1);
        assert_eq!(
            report.planned[0]
                .to
                .file_name()
                .and_then(|name| name.to_str()),
            Some("landscape-wallpaper1.png")
        );
    }
}
