use crate::app::Config;
use crate::collections::CollectionStore;
use crate::pairing::PairingHistory;
use crate::screen::AspectCategory;
use crate::thumbnail::ThumbnailCache;
use crate::utils::is_image_file;
use crate::wallpaper::{Wallpaper, WallpaperCache};
use anyhow::{Context, Result};
use clap::ValueEnum;
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const NATIVE_BUCKETS: [RenameBucket; 4] = [
    RenameBucket::Ultrawide,
    RenameBucket::Landscape,
    RenameBucket::Portrait,
    RenameBucket::Square,
];
const LEGACY_BUCKETS: [RenameBucket; 3] = [
    RenameBucket::Widescreen,
    RenameBucket::Portrait,
    RenameBucket::Square,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum, Default)]
pub enum RenameScheme {
    #[default]
    Native,
    Legacy,
}

#[derive(Debug, Clone, Copy)]
pub struct RenameOptions {
    pub dry_run: bool,
    pub compact: bool,
    pub warn_content_dupes: bool,
    pub scheme: RenameScheme,
}

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
struct WallpaperEntry {
    path: PathBuf,
    extension: String,
    bucket: RenameBucket,
    current_name: Option<(RenameBucket, u32)>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum RenameBucket {
    Ultrawide,
    Landscape,
    Portrait,
    Square,
    Widescreen,
}

impl RenameBucket {
    fn prefix(self) -> &'static str {
        match self {
            RenameBucket::Ultrawide => "ultrawide",
            RenameBucket::Landscape => "landscape",
            RenameBucket::Portrait => "portrait",
            RenameBucket::Square => "square",
            RenameBucket::Widescreen => "widescreen",
        }
    }
}

impl RenameScheme {
    fn supported_buckets(self) -> &'static [RenameBucket] {
        match self {
            RenameScheme::Native => &NATIVE_BUCKETS,
            RenameScheme::Legacy => &LEGACY_BUCKETS,
        }
    }

    fn bucket_for_aspect(self, aspect: AspectCategory) -> RenameBucket {
        match self {
            RenameScheme::Native => match aspect {
                AspectCategory::Ultrawide => RenameBucket::Ultrawide,
                AspectCategory::Landscape => RenameBucket::Landscape,
                AspectCategory::Portrait => RenameBucket::Portrait,
                AspectCategory::Square => RenameBucket::Square,
            },
            RenameScheme::Legacy => match aspect {
                AspectCategory::Portrait => RenameBucket::Portrait,
                AspectCategory::Square => RenameBucket::Square,
                AspectCategory::Ultrawide | AspectCategory::Landscape => RenameBucket::Widescreen,
            },
        }
    }
}

impl Default for RenameOptions {
    fn default() -> Self {
        Self {
            dry_run: false,
            compact: false,
            warn_content_dupes: false,
            scheme: RenameScheme::Native,
        }
    }
}

pub fn rename_wallpapers(dir: &Path, options: RenameOptions) -> Result<RenameReport> {
    let mut report = plan_rename_operations(dir, &options)?;
    if options.dry_run || report.planned.is_empty() {
        return Ok(report);
    }

    let stale_thumbnail_names: Vec<String> = report
        .planned
        .iter()
        .map(|rename| ThumbnailCache::cache_file_name_for_source(&rename.from))
        .collect();

    execute_plan(&report.planned)?;

    let mapping: HashMap<PathBuf, PathBuf> = report
        .planned
        .iter()
        .map(|rename| (rename.from.clone(), rename.to.clone()))
        .collect();

    let mut config = Config::load()?;
    report.migration.session_paths_updated = config.remap_session_paths(&mapping)?;
    report.migration.cache_files_updated = WallpaperCache::remap_persisted_paths(&mapping)?;

    let mut collections = CollectionStore::load()?;
    report.migration.collection_paths_updated = collections.remap_paths(&mapping)?;

    let mut history = PairingHistory::load(config.pairing.max_history_records)
        .unwrap_or_else(|_| PairingHistory::new(config.pairing.max_history_records));
    report.migration.pairing_paths_updated = history.remap_paths(&mapping)?;

    report.migration.thumbnails_removed =
        ThumbnailCache::purge_cache_file_names(&stale_thumbnail_names);

    Ok(report)
}

fn plan_rename_operations(dir: &Path, options: &RenameOptions) -> Result<RenameReport> {
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

fn detect_duplicate_numbered_stems(entries: &[WallpaperEntry]) -> Vec<String> {
    let mut seen: HashMap<(RenameBucket, u32), PathBuf> = HashMap::new();
    let mut warnings = Vec::new();

    for entry in entries {
        let Some((bucket, number)) = entry.current_name else {
            continue;
        };
        let key = (bucket, number);
        if let Some(first) = seen.get(&key) {
            warnings.push(format!(
                "{}-wallpaper{}: {} <-> {}",
                bucket.prefix(),
                number,
                display_name(first),
                display_name(&entry.path)
            ));
        } else {
            seen.insert(key, entry.path.clone());
        }
    }

    warnings
}

fn detect_content_duplicates(entries: &[WallpaperEntry]) -> Vec<String> {
    let hashes: Vec<_> = entries
        .par_iter()
        .filter_map(|entry| {
            file_sha256(&entry.path)
                .ok()
                .map(|hash| (hash, entry.path.clone()))
        })
        .collect();

    let mut first_by_hash: HashMap<String, PathBuf> = HashMap::new();
    let mut warnings = Vec::new();

    for (hash, path) in hashes {
        if let Some(first) = first_by_hash.get(&hash) {
            warnings.push(format!(
                "{} == {}",
                display_name(first),
                display_name(&path)
            ));
        } else {
            first_by_hash.insert(hash, path);
        }
    }

    warnings
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
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

fn execute_plan(plan: &[PlannedRename]) -> Result<()> {
    if plan.is_empty() {
        return Ok(());
    }

    let mut stages = Vec::with_capacity(plan.len());
    let pid = std::process::id();

    for (idx, rename) in plan.iter().enumerate() {
        let parent = rename
            .from
            .parent()
            .context("Wallpaper path had no parent directory")?;
        let extension = rename
            .from
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("tmp");
        let mut temp_path = parent.join(format!(".frostwall-rename-{pid}-{idx}.{extension}"));
        let mut suffix = 0_u32;
        while temp_path.exists() {
            suffix += 1;
            temp_path = parent.join(format!(
                ".frostwall-rename-{pid}-{idx}-{suffix}.{extension}"
            ));
        }

        stages.push(RenameStage {
            original: rename.from.clone(),
            temp: temp_path,
            final_path: rename.to.clone(),
        });
    }

    let mut phase_one_complete = 0;
    for stage in &stages {
        if let Err(error) = fs::rename(&stage.original, &stage.temp) {
            rollback_stages(&stages, phase_one_complete, 0);
            return Err(error).with_context(|| {
                format!(
                    "Failed to move {} to temporary path",
                    stage.original.display()
                )
            });
        }
        phase_one_complete += 1;
    }

    let mut finalized = 0;
    for stage in &stages {
        if let Err(error) = fs::rename(&stage.temp, &stage.final_path) {
            rollback_stages(&stages, phase_one_complete, finalized);
            return Err(error).with_context(|| {
                format!(
                    "Failed to finalize rename {} -> {}",
                    stage.original.display(),
                    stage.final_path.display()
                )
            });
        }
        finalized += 1;
    }

    Ok(())
}

fn rollback_stages(stages: &[RenameStage], phase_one_complete: usize, finalized: usize) {
    for stage in stages[..finalized].iter().rev() {
        if stage.final_path.exists() {
            let _ = fs::rename(&stage.final_path, &stage.original);
        }
    }

    for stage in stages[finalized..phase_one_complete].iter().rev() {
        if stage.temp.exists() {
            let _ = fs::rename(&stage.temp, &stage.original);
        }
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}

struct RenameStage {
    original: PathBuf,
    temp: PathBuf,
    final_path: PathBuf,
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

    #[test]
    fn execute_plan_renames_files_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("source.png");
        let to = dir.path().join("landscape-wallpaper1.png");
        write_png(&from, 1920, 1080);

        execute_plan(&[PlannedRename {
            from: from.clone(),
            to: to.clone(),
        }])
        .unwrap();

        assert!(!from.exists());
        assert!(to.exists());
    }
}
