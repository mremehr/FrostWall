use super::{CacheLoadMode, CacheStats, Wallpaper, WallpaperCache, CACHE_VERSION};
use crate::screen::{AspectCategory, Screen};
use anyhow::{Context, Result};
use rand::Rng;
use rayon::prelude::*;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

impl WallpaperCache {
    fn cache_path() -> PathBuf {
        directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("wallpaper_cache.json")
    }

    pub fn load_or_scan_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        Self::load_or_scan_with_mode(source_dir, recursive, CacheLoadMode::Full)
    }

    pub fn load_or_scan_for_ai_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        Self::load_or_scan_with_mode(source_dir, recursive, CacheLoadMode::MetadataOnly)
    }

    fn load_or_scan_with_mode(
        source_dir: &Path,
        recursive: bool,
        mode: CacheLoadMode,
    ) -> Result<Self> {
        let cache_path = Self::cache_path();

        if cache_path.exists() {
            let data = fs::read_to_string(&cache_path)?;
            if let Ok(cache) = serde_json::from_str::<WallpaperCache>(&data) {
                if cache.version != CACHE_VERSION {
                    eprintln!(
                        "Cache format changed (v{} -> v{}), rescanning...",
                        cache.version, CACHE_VERSION
                    );
                    return Self::scan_with_mode(source_dir, recursive, mode);
                }
                let valid = match mode {
                    CacheLoadMode::Full => cache.validate(),
                    CacheLoadMode::MetadataOnly => cache.validate_for_ai(),
                };
                if cache.source_dir == source_dir && valid {
                    return Ok(cache);
                }
            }
        }

        Self::scan_with_mode(source_dir, recursive, mode)
    }

    fn scan_with_mode(source_dir: &Path, recursive: bool, mode: CacheLoadMode) -> Result<Self> {
        match mode {
            CacheLoadMode::Full => Self::scan_recursive(source_dir, recursive),
            CacheLoadMode::MetadataOnly => {
                Self::scan_metadata_only_recursive(source_dir, recursive)
            }
        }
    }

    pub fn scan_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        let entries: Vec<PathBuf> = if recursive {
            // Use walkdir for recursive scanning
            WalkDir::new(source_dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .map(|e| e.path().to_path_buf())
                .filter(|p| p.is_file() && crate::utils::is_image_file(p))
                .collect()
        } else {
            // Non-recursive: just read the directory
            fs::read_dir(source_dir)
                .with_context(|| format!("Failed to read directory: {}", source_dir.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && crate::utils::is_image_file(p))
                .collect()
        };

        let total = entries.len();
        let processed = AtomicUsize::new(0);

        // Phase 1: Fast parallel scan (header only - dimensions)
        eprint!("Phase 1/2: Reading dimensions...");
        let mut wallpapers: Vec<Wallpaper> = entries
            .par_iter()
            .filter_map(|path| {
                let count = processed.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_multiple_of(50) || count == total {
                    eprint!("\rPhase 1/2: Reading dimensions... {}/{}", count, total);
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

        // Phase 2: Batched parallel color extraction (10 at a time)
        let color_total = wallpapers.len();
        const BATCH_SIZE: usize = 10;

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

        // Sort by filename for consistent ordering
        wallpapers.sort_by(|a, b| a.path.cmp(&b.path));

        Ok(Self {
            version: CACHE_VERSION,
            wallpapers,
            source_dir: source_dir.to_path_buf(),
            screen_indices: HashMap::new(),
            recursive,
        })
    }

    /// Fast scan for AI operations (dimensions + metadata only, no color extraction).
    pub fn scan_metadata_only_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        let entries: Vec<PathBuf> = if recursive {
            WalkDir::new(source_dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .map(|e| e.path().to_path_buf())
                .filter(|p| p.is_file() && crate::utils::is_image_file(p))
                .collect()
        } else {
            fs::read_dir(source_dir)
                .with_context(|| format!("Failed to read directory: {}", source_dir.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && crate::utils::is_image_file(p))
                .collect()
        };

        let total = entries.len();
        let processed = AtomicUsize::new(0);

        eprint!("Phase 1/1: Reading metadata...");
        let mut wallpapers: Vec<Wallpaper> = entries
            .par_iter()
            .filter_map(|path| {
                let count = processed.fetch_add(1, Ordering::Relaxed) + 1;
                if count.is_multiple_of(50) || count == total {
                    eprint!("\rPhase 1/1: Reading metadata... {}/{}", count, total);
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

        Ok(Self {
            version: CACHE_VERSION,
            wallpapers,
            source_dir: source_dir.to_path_buf(),
            screen_indices: HashMap::new(),
            recursive,
        })
    }

    /// Incremental rescan: discover new files and remove deleted ones while
    /// preserving all existing data (tags, auto_tags, embeddings, colors).
    /// Returns (added, removed) counts.
    pub fn incremental_rescan(&mut self, recursive: bool) -> Result<(usize, usize)> {
        let source_dir = self.source_dir.clone();

        // Discover current files on disk
        let on_disk: Vec<PathBuf> = if recursive {
            WalkDir::new(&source_dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .map(|e| e.path().to_path_buf())
                .filter(|p| p.is_file() && crate::utils::is_image_file(p))
                .collect()
        } else {
            fs::read_dir(&source_dir)
                .with_context(|| format!("Failed to read directory: {}", source_dir.display()))?
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file() && crate::utils::is_image_file(p))
                .collect()
        };

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
                // Existing file — check if modified since last scan
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
                    // Re-read dimensions + colors but preserve tags/embeddings
                    match Wallpaper::from_path_fast(path) {
                        Ok(mut fresh) => {
                            // Preserve user data
                            fresh.tags = wp.tags;
                            fresh.auto_tags = wp.auto_tags;
                            fresh.embedding = wp.embedding;
                            // Re-extract colors for modified file
                            let _ = fresh.extract_colors();
                            kept.push(fresh);
                        }
                        Err(_) => kept.push(wp), // Keep old data on error
                    }
                } else {
                    kept.push(wp);
                }
            } else {
                // New file — scan dimensions + colors
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

        // Whatever remains in `existing` was deleted from disk
        let removed = existing.len();

        kept.sort_by(|a, b| a.path.cmp(&b.path));
        self.wallpapers = kept;
        self.version = CACHE_VERSION;

        // Auto-save after rescan
        self.save()?;

        Ok((added, removed))
    }

    pub fn save(&self) -> Result<()> {
        let cache_path = Self::cache_path();

        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = serde_json::to_string_pretty(self)?;
        fs::write(&cache_path, data)?;

        Ok(())
    }

    fn validate(&self) -> bool {
        self.validate_impl(true)
    }

    fn validate_for_ai(&self) -> bool {
        self.validate_impl(false)
    }

    fn validate_impl(&self, require_color_data: bool) -> bool {
        // Check if source directory still exists
        if !self.source_dir.exists() {
            return false;
        }

        // Check a sample of files (up to 20) for existence and modification time
        let sample_size = self.wallpapers.len().min(20);
        let step = if self.wallpapers.len() > sample_size {
            self.wallpapers.len() / sample_size
        } else {
            1
        };

        for (i, wp) in self.wallpapers.iter().enumerate() {
            // Check every Nth file to get a representative sample
            if i % step != 0 {
                continue;
            }

            // File must exist
            if !wp.path.exists() {
                return false;
            }

            // Full runtime needs color data; AI tagging path does not.
            if require_color_data && wp.colors.is_empty() {
                return false;
            }

            // Check if file was modified since caching (if we have mtime)
            if wp.modified_at > 0 {
                if let Ok(meta) = std::fs::metadata(&wp.path) {
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                            let current_mtime = duration.as_secs();
                            // If file was modified after cache, invalidate
                            if current_mtime > wp.modified_at {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        // Quick check: count files in directory to detect additions/removals
        let current_count = if self.recursive {
            WalkDir::new(&self.source_dir)
                .follow_links(true)
                .into_iter()
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file() && crate::utils::is_image_file(e.path()))
                .count()
        } else if let Ok(entries) = std::fs::read_dir(&self.source_dir) {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file() && crate::utils::is_image_file(&e.path()))
                .count()
        } else {
            return false;
        };

        if current_count != self.wallpapers.len() {
            return false;
        }

        true
    }

    pub fn for_screen(&self, screen: &Screen) -> Vec<&Wallpaper> {
        self.wallpapers
            .iter()
            .filter(|wp| wp.matches_screen(screen))
            .collect()
    }

    pub fn random_for_screen(&self, screen: &Screen) -> Option<&Wallpaper> {
        let matching: Vec<_> = self.for_screen(screen);
        if matching.is_empty() {
            // Fallback: any wallpaper
            if self.wallpapers.is_empty() {
                return None;
            }
            let idx = rand::thread_rng().gen_range(0..self.wallpapers.len());
            return Some(&self.wallpapers[idx]);
        }

        let idx = rand::thread_rng().gen_range(0..matching.len());
        Some(matching[idx])
    }

    pub fn next_for_screen(&mut self, screen: &Screen) -> Option<&Wallpaper> {
        let matching: Vec<_> = self
            .wallpapers
            .iter()
            .enumerate()
            .filter(|(_, wp)| wp.matches_screen(screen))
            .collect();

        if matching.is_empty() {
            return None;
        }

        let current = self
            .screen_indices
            .get(&screen.name)
            .copied()
            .unwrap_or_else(|| matching.len().saturating_sub(1));
        let next = (current + 1) % matching.len();
        self.screen_indices.insert(screen.name.clone(), next);

        Some(matching[next].1)
    }

    pub fn prev_for_screen(&mut self, screen: &Screen) -> Option<&Wallpaper> {
        let matching: Vec<_> = self
            .wallpapers
            .iter()
            .enumerate()
            .filter(|(_, wp)| wp.matches_screen(screen))
            .collect();

        if matching.is_empty() {
            return None;
        }

        let current = self.screen_indices.get(&screen.name).copied().unwrap_or(0);
        let prev = if current == 0 {
            matching.len() - 1
        } else {
            current - 1
        };
        self.screen_indices.insert(screen.name.clone(), prev);

        Some(matching[prev].1)
    }

    pub fn stats(&self) -> CacheStats {
        let mut stats = CacheStats {
            total: self.wallpapers.len(),
            ..Default::default()
        };

        for wp in &self.wallpapers {
            match wp.aspect_category {
                AspectCategory::Ultrawide => stats.ultrawide += 1,
                AspectCategory::Landscape => stats.landscape += 1,
                AspectCategory::Portrait => stats.portrait += 1,
                AspectCategory::Square => stats.square += 1,
            }
        }

        stats
    }

    /// Get all unique tags across all wallpapers
    pub fn all_tags(&self) -> Vec<String> {
        let mut tags: Vec<String> = self
            .wallpapers
            .iter()
            .flat_map(|wp| wp.all_tags())
            .collect();
        tags.sort();
        tags.dedup();
        tags
    }

    /// Add a tag to a wallpaper by path
    pub fn add_tag(&mut self, path: &Path, tag: &str) -> bool {
        if let Some(wp) = self.wallpapers.iter_mut().find(|w| w.path == path) {
            wp.add_tag(tag);
            true
        } else {
            false
        }
    }

    /// Remove a tag from a wallpaper by path
    pub fn remove_tag(&mut self, path: &Path, tag: &str) -> bool {
        if let Some(wp) = self.wallpapers.iter_mut().find(|w| w.path == path) {
            wp.remove_tag(tag);
            true
        } else {
            false
        }
    }

    /// Get wallpapers with specific tag
    pub fn with_tag(&self, tag: &str) -> Vec<&Wallpaper> {
        self.wallpapers
            .iter()
            .filter(|wp| wp.has_tag(tag))
            .collect()
    }
}
