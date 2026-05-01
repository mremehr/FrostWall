use crate::wallpaper::{Wallpaper, WallpaperCache, CACHE_VERSION};
use anyhow::{Context, Result};
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

/// Collect image paths from a directory, optionally recursive.
fn collect_image_paths(source_dir: &Path, recursive: bool) -> Result<Vec<PathBuf>> {
    if recursive {
        Ok(WalkDir::new(source_dir)
            .follow_links(true)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|entry| entry.file_type().is_file())
            .map(walkdir::DirEntry::into_path)
            .filter(|path| crate::utils::is_image_file(path))
            .collect())
    } else {
        fs::read_dir(source_dir)
            .with_context(|| format!("Failed to read directory: {}", source_dir.display()))
            .map(|rd| {
                rd.flatten()
                    .map(|e| e.path())
                    .filter(|p| p.is_file() && crate::utils::is_image_file(p))
                    .collect()
            })
    }
}

fn scan_wallpaper_metadata(entries: &[PathBuf], progress_label: &str) -> Vec<Wallpaper> {
    let total = entries.len();
    let processed = AtomicUsize::new(0);

    eprint!("{progress_label}...");
    let mut wallpapers: Vec<Wallpaper> = entries
        .par_iter()
        .filter_map(|path| {
            let count = processed.fetch_add(1, Ordering::Relaxed) + 1;
            if count.is_multiple_of(50) || count == total {
                eprint!("\r{progress_label}... {count}/{total}");
            }
            match Wallpaper::from_path_fast(path) {
                Ok(wp) => Some(wp),
                Err(e) => {
                    eprintln!("\nWarning: Failed to read {}: {}", path.display(), e);
                    None
                }
            }
        })
        .collect();
    eprintln!(" done!");

    wallpapers.sort_by(|a, b| a.path.cmp(&b.path));
    wallpapers
}

fn extract_colors_in_batches(wallpapers: &mut [Wallpaper]) {
    const BATCH_SIZE: usize = 10;
    let color_total = wallpapers.len();

    for (batch_idx, chunk) in wallpapers.chunks_mut(BATCH_SIZE).enumerate() {
        let batch_start = batch_idx * BATCH_SIZE;
        chunk.par_iter_mut().for_each(|wp| {
            if let Err(e) = wp.extract_colors() {
                eprintln!(
                    "\nWarning: Failed to extract colors for {}: {}",
                    wp.path.display(),
                    e
                );
            }
        });
        let progress = (batch_start + chunk.len()).min(color_total);
        eprint!(
            "\rPhase 2/2: Extracting colors... {}/{}",
            progress, color_total
        );
    }
    eprintln!(" done!");
}

fn build_cache(source_dir: &Path, recursive: bool, wallpapers: Vec<Wallpaper>) -> WallpaperCache {
    let mut cache = WallpaperCache {
        version: CACHE_VERSION,
        wallpapers,
        source_dir: source_dir.to_path_buf(),
        screen_indices: HashMap::new(),
        recursive,
        screen_match_indices: HashMap::new(),
        similarity_profiles: Vec::new(),
    };
    cache.rebuild_similarity_profiles();
    cache
}

impl WallpaperCache {
    pub fn scan_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        let entries = collect_image_paths(source_dir, recursive)?;
        let mut wallpapers = scan_wallpaper_metadata(&entries, "Phase 1/2: Reading dimensions");
        extract_colors_in_batches(&mut wallpapers);
        Ok(build_cache(source_dir, recursive, wallpapers))
    }

    /// Fast scan for AI operations (dimensions + metadata only, no color extraction).
    pub fn scan_metadata_only_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        let entries = collect_image_paths(source_dir, recursive)?;
        let wallpapers = scan_wallpaper_metadata(&entries, "Phase 1/1: Reading metadata");
        Ok(build_cache(source_dir, recursive, wallpapers))
    }

    /// Incremental rescan: discover new files and remove deleted ones while
    /// preserving all existing data (tags, auto_tags, embeddings, colors).
    /// Returns (added, removed) counts.
    pub fn incremental_rescan(&mut self, recursive: bool) -> Result<(usize, usize)> {
        let source_dir = self.source_dir.clone();
        let on_disk = collect_image_paths(&source_dir, recursive)?;

        // Build lookup of existing wallpapers by path
        let mut existing: HashMap<PathBuf, Wallpaper> = self
            .wallpapers
            .drain(..)
            .map(|wp| (wp.path.clone(), wp))
            .collect();

        let mut added = 0usize;
        let mut kept = Vec::with_capacity(on_disk.len());

        for path in &on_disk {
            if let Some(wp) = existing.remove(path) {
                let needs_refresh = if wp.modified_at > 0 {
                    std::fs::metadata(path)
                        .ok()
                        .and_then(|m| m.modified().ok())
                        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .map(|d| d.as_secs() > wp.modified_at)
                        .unwrap_or(false)
                } else {
                    false
                };

                if needs_refresh {
                    match Wallpaper::from_path_fast(path) {
                        Ok(mut fresh) => {
                            fresh.tags = wp.tags;
                            fresh.auto_tags = wp.auto_tags;
                            fresh.embedding = wp.embedding;
                            let _ = fresh.extract_colors();
                            kept.push(fresh);
                        }
                        Err(_) => kept.push(wp),
                    }
                } else {
                    kept.push(wp);
                }
            } else {
                match Wallpaper::from_path_fast(path) {
                    Ok(mut wp) => {
                        let _ = wp.extract_colors();
                        kept.push(wp);
                        added += 1;
                    }
                    Err(e) => {
                        eprintln!("Warning: Failed to read {}: {}", path.display(), e);
                    }
                }
            }
        }

        let removed = existing.len();

        kept.sort_by(|a, b| a.path.cmp(&b.path));
        self.wallpapers = kept;
        self.version = CACHE_VERSION;
        self.screen_match_indices.clear();
        self.rebuild_similarity_profiles();
        self.save()?;

        Ok((added, removed))
    }

    pub(super) fn ensure_full_color_data(&mut self) -> bool {
        let missing = self
            .wallpapers
            .iter()
            .filter(|wp| wp.colors.is_empty())
            .count();

        if missing == 0 {
            return false;
        }

        let processed = AtomicUsize::new(0);
        eprint!("Backfilling missing colors... 0/{}", missing);
        self.wallpapers.par_iter_mut().for_each(|wp| {
            if wp.colors.is_empty() {
                if let Err(err) = wp.extract_colors() {
                    eprintln!(
                        "\nWarning: Failed to extract colors for {}: {}",
                        wp.path.display(),
                        err
                    );
                }
                let count = processed.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_multiple_of(25) || count == missing {
                    eprint!("\rBackfilling missing colors... {}/{}", count, missing);
                }
            }
        });
        eprintln!(" done!");

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_test_image(dir: &std::path::Path, name: &str) {
        // Create a minimal valid PNG (1x1 white pixel).
        let png_bytes: &[u8] = &[
            0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, // PNG signature
            0x00, 0x00, 0x00, 0x0d, 0x49, 0x48, 0x44, 0x52, // IHDR chunk length + type
            0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, // width=1, height=1
            0x08, 0x02, 0x00, 0x00, 0x00, 0x90, 0x77, 0x53, // bit depth=8, colortype=2
            0xde, 0x00, 0x00, 0x00, 0x0c, 0x49, 0x44, 0x41, // IDAT chunk
            0x54, 0x08, 0xd7, 0x63, 0xf8, 0xff, 0xff, 0x3f, 0x00, 0x05, 0xfe, 0x02, 0xfe, 0xdc,
            0xcc, 0x59, 0xe7, 0x00, 0x00, 0x00, 0x00, 0x49, 0x45, 0x4e, // IEND chunk
            0x44, 0xae, 0x42, 0x60, 0x82,
        ];
        fs::write(dir.join(name), png_bytes).expect("write test image");
    }

    #[test]
    fn test_scan_recursive_finds_images() {
        let dir = tempfile::tempdir().expect("create tempdir");
        make_test_image(dir.path(), "a.png");
        make_test_image(dir.path(), "b.png");

        let cache = WallpaperCache::scan_recursive(dir.path(), false).expect("scan should succeed");

        assert_eq!(cache.wallpapers.len(), 2, "should find 2 images");
        assert!(cache
            .wallpapers
            .iter()
            .any(|w| w.path.file_name().unwrap() == "a.png"));
        assert!(cache
            .wallpapers
            .iter()
            .any(|w| w.path.file_name().unwrap() == "b.png"));
    }

    #[test]
    fn test_incremental_rescan_preserves_tags() {
        let dir = tempfile::tempdir().expect("create tempdir");
        make_test_image(dir.path(), "tagged.png");

        let mut cache = WallpaperCache::scan_recursive(dir.path(), false).expect("initial scan");

        // Tag the wallpaper manually.
        let path = cache.wallpapers[0].path.clone();
        cache.wallpapers[0].add_tag("my-tag");
        assert!(cache.wallpapers[0].has_tag("my-tag"), "tag must be set");

        // Add a new image and rescan.
        make_test_image(dir.path(), "new.png");
        let (added, removed) = cache.incremental_rescan(false).expect("rescan");

        assert_eq!(added, 1, "one new image added");
        assert_eq!(removed, 0, "no images removed");

        // Original wallpaper's tag must survive.
        let tagged = cache
            .wallpapers
            .iter()
            .find(|w| w.path == path)
            .expect("original wallpaper still present");
        assert!(
            tagged.has_tag("my-tag"),
            "tag must be preserved after rescan"
        );
    }

    #[test]
    fn test_incremental_rescan_removes_deleted_files() {
        let dir = tempfile::tempdir().expect("create tempdir");
        make_test_image(dir.path(), "keep.png");
        make_test_image(dir.path(), "delete.png");

        let mut cache = WallpaperCache::scan_recursive(dir.path(), false).expect("initial scan");
        assert_eq!(cache.wallpapers.len(), 2);

        // Delete one file.
        fs::remove_file(dir.path().join("delete.png")).expect("remove file");

        let (added, removed) = cache
            .incremental_rescan(false)
            .expect("rescan after delete");

        assert_eq!(added, 0);
        assert_eq!(removed, 1, "one image should be removed");
        assert_eq!(cache.wallpapers.len(), 1, "only one wallpaper remains");
        assert!(cache.wallpapers[0].path.file_name().unwrap() == "keep.png");
    }

    #[test]
    fn test_cache_save_load_roundtrip() {
        let dir = tempfile::tempdir().expect("create tempdir");
        make_test_image(dir.path(), "roundtrip.png");
        let cache =
            WallpaperCache::scan_recursive(dir.path(), false).expect("scan_recursive should work");

        assert!(
            !cache.wallpapers.is_empty(),
            "should contain at least one wallpaper"
        );
        let save_path = dir.path().join("cache.json");
        cache
            .save_to(&save_path)
            .expect("save_to should persist in tempdir");
        let roundtrip = WallpaperCache::load_from(&save_path).expect("load_from should work");
        assert_eq!(roundtrip.wallpapers.len(), cache.wallpapers.len());
    }
}
