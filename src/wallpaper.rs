use crate::clip::AutoTag;
use crate::screen::{AspectCategory, Screen};
use anyhow::{Context, Result};
use image::imageops::FilterType;
use kmeans_colors::get_kmeans_hamerly;
use palette::{IntoColor, Lab, Srgb};
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use walkdir::WalkDir;

/// How strictly to match wallpaper aspect ratio to screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MatchMode {
    /// Only exact aspect category match
    #[default]
    Strict,
    /// Flexible: landscape works on ultrawide, portrait on portrait
    Flexible,
    /// Show all wallpapers regardless of aspect ratio
    All,
}

/// Sort order for wallpapers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortMode {
    /// Sort by filename (A-Z)
    #[default]
    Name,
    /// Sort by image dimensions (largest first)
    Size,
    /// Sort by modification date (newest first)
    Date,
}

impl SortMode {
    /// Return human-readable display name for this sort mode.
    pub fn display_name(&self) -> &'static str {
        match self {
            SortMode::Name => "Name",
            SortMode::Size => "Size",
            SortMode::Date => "Date",
        }
    }

    /// Cycle to the next sort mode.
    pub fn next(&self) -> Self {
        match self {
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::Date,
            SortMode::Date => SortMode::Name,
        }
    }
}

impl MatchMode {
    /// Return human-readable display name for this match mode.
    pub fn display_name(&self) -> &'static str {
        match self {
            MatchMode::Strict => "Strict",
            MatchMode::Flexible => "Flexible",
            MatchMode::All => "All",
        }
    }

    /// Cycle to the next match mode.
    pub fn next(&self) -> Self {
        match self {
            MatchMode::Strict => MatchMode::Flexible,
            MatchMode::Flexible => MatchMode::All,
            MatchMode::All => MatchMode::Strict,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallpaper {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub aspect_category: AspectCategory,
    pub colors: Vec<String>,
    /// User-defined tags for this wallpaper
    #[serde(default)]
    pub tags: Vec<String>,
    /// CLIP-generated auto tags with confidence scores
    #[serde(default)]
    pub auto_tags: Vec<AutoTag>,
    /// Color weights/proportions (how much of the image each color represents, 0.0-1.0)
    #[serde(default)]
    pub color_weights: Vec<f32>,
    /// Cached CLIP embedding for similarity search (512 dimensions)
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
    /// File size in bytes (for sorting)
    #[serde(default)]
    pub file_size: u64,
    /// Modification timestamp (seconds since epoch, for sorting)
    #[serde(default)]
    pub modified_at: u64,
}

/// Current cache format version — bump when the serialized shape changes
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperCache {
    /// Cache format version for forward-compatible migration
    #[serde(default)]
    pub version: u32,
    pub wallpapers: Vec<Wallpaper>,
    pub source_dir: PathBuf,
    /// Track current index per screen for next/prev
    #[serde(default)]
    pub screen_indices: HashMap<String, usize>,
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub total: usize,
    pub ultrawide: usize,
    pub landscape: usize,
    pub portrait: usize,
    pub square: usize,
}

impl Wallpaper {
    /// Fast path: only read dimensions from image header (no full decode)
    pub fn from_path_fast(path: &Path) -> Result<Self> {
        // Only read image header - much faster than full decode!
        let (width, height) =
            image::image_dimensions(path).context("Failed to read image dimensions")?;
        let aspect_category = Self::categorize_aspect(width, height);

        // Get file metadata for sorting
        let metadata = std::fs::metadata(path).ok();
        let file_size = metadata.as_ref().map(|m| m.len()).unwrap_or(0);
        let modified_at = metadata
            .and_then(|m| m.modified().ok())
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_secs())
            .unwrap_or(0);

        Ok(Self {
            path: path.to_path_buf(),
            width,
            height,
            aspect_category,
            colors: Vec::new(), // Colors extracted lazily
            color_weights: Vec::new(),
            tags: Vec::new(),
            auto_tags: Vec::new(),
            embedding: None,
            file_size,
            modified_at,
        })
    }

    /// Extract colors for a wallpaper (call after from_path_fast if colors needed)
    pub fn extract_colors(&mut self) -> Result<()> {
        if !self.colors.is_empty() {
            return Ok(()); // Already extracted
        }

        const K: usize = 5;
        const CONVERGENCE_THRESHOLD: f32 = 5.0; // Looser convergence (was 2.0)
        const MAX_ITERATIONS: u32 = 30; // Faster (was 100)
        const THUMBNAIL_SIZE: u32 = 128; // Smaller (was 256)

        let img = image::open(&self.path).context("Failed to open image")?;
        let thumb = img.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Triangle);
        let pixels: Vec<_> = thumb.to_rgb8().pixels().cloned().collect();

        let lab: Vec<Lab> = pixels
            .par_iter()
            .map(|p| {
                let rgb = Srgb::new(
                    p.0[0] as f32 / 255.0,
                    p.0[1] as f32 / 255.0,
                    p.0[2] as f32 / 255.0,
                );
                rgb.into_color()
            })
            .collect();

        let result = get_kmeans_hamerly(
            K,
            MAX_ITERATIONS as usize,
            CONVERGENCE_THRESHOLD,
            false,
            &lab,
            0,
        );

        // Calculate color weights (proportion of image each color represents)
        let total_pixels = lab.len() as f32;
        let mut counts = [0usize; K];
        for &idx in &result.indices {
            counts[idx as usize] += 1;
        }

        // Create paired colors and weights, then sort by weight descending
        let mut color_weight_pairs: Vec<(String, f32)> = result
            .centroids
            .iter()
            .zip(counts.iter())
            .map(|(c, &count)| {
                let rgb: Srgb = (*c).into_color();
                let r = (rgb.red * 255.0) as u8;
                let g = (rgb.green * 255.0) as u8;
                let b = (rgb.blue * 255.0) as u8;
                let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
                let weight = count as f32 / total_pixels;
                (hex, weight)
            })
            .collect();

        // Sort by weight descending (most dominant color first)
        color_weight_pairs
            .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

        // Unzip into separate vectors
        self.colors = color_weight_pairs.iter().map(|(c, _)| c.clone()).collect();
        self.color_weights = color_weight_pairs.iter().map(|(_, w)| *w).collect();

        // Note: Auto-tags are now generated via CLIP (frostwall auto-tag command)
        // and not automatically from color extraction

        Ok(())
    }

    /// Full path with colors (legacy, slower)
    pub fn from_path(path: &Path) -> Result<Self> {
        let mut wp = Self::from_path_fast(path)?;
        wp.extract_colors()?;
        Ok(wp)
    }

    pub(crate) fn categorize_aspect(width: u32, height: u32) -> AspectCategory {
        let ratio = width as f32 / height as f32;
        let normalized_ratio = if ratio >= 1.0 { ratio } else { 1.0 / ratio };

        if normalized_ratio >= 2.0 {
            AspectCategory::Ultrawide
        } else if normalized_ratio >= 1.2 {
            if ratio >= 1.0 {
                AspectCategory::Landscape
            } else {
                AspectCategory::Portrait
            }
        } else {
            AspectCategory::Square
        }
    }

    /// Strict match - exact aspect category
    pub fn matches_screen(&self, screen: &Screen) -> bool {
        self.aspect_category == screen.aspect_category
    }

    /// Flexible match - allows compatible aspect ratios
    /// - Landscape wallpapers work on Ultrawide screens (will be cropped/padded)
    /// - Portrait wallpapers work on Portrait screens
    /// - Square works with everything
    pub fn matches_screen_flexible(&self, screen: &Screen) -> bool {
        use AspectCategory::*;

        match (self.aspect_category, screen.aspect_category) {
            // Exact match always works
            (a, b) if a == b => true,

            // Landscape wallpapers can be used on ultrawide (crop sides or pad)
            (Landscape, Ultrawide) => true,
            // Ultrawide wallpapers can work on landscape (crop or pad top/bottom)
            (Ultrawide, Landscape) => true,

            // Square is versatile - works with landscape orientations
            (Square, Landscape) | (Square, Ultrawide) => true,
            (Landscape, Square) | (Ultrawide, Square) => true,

            // Portrait stays with portrait (or square)
            (Portrait, Square) | (Square, Portrait) => true,

            // Don't mix landscape/ultrawide with portrait
            _ => false,
        }
    }

    /// Match based on mode
    pub fn matches_screen_with_mode(&self, screen: &Screen, mode: MatchMode) -> bool {
        match mode {
            MatchMode::Strict => self.matches_screen(screen),
            MatchMode::Flexible => self.matches_screen_flexible(screen),
            MatchMode::All => true,
        }
    }

    /// Add a tag to this wallpaper
    pub fn add_tag(&mut self, tag: &str) {
        let tag = tag.to_lowercase().trim().to_string();
        if !tag.is_empty() && !self.tags.contains(&tag) {
            self.tags.push(tag);
            self.tags.sort();
        }
    }

    /// Remove a tag from this wallpaper
    pub fn remove_tag(&mut self, tag: &str) {
        let tag = tag.to_lowercase();
        self.tags.retain(|t| t != &tag);
    }

    /// Check if wallpaper has a specific tag (manual or auto)
    pub fn has_tag(&self, tag: &str) -> bool {
        let tag = tag.to_lowercase();
        self.tags.iter().any(|t| t == &tag)
            || self.auto_tags.iter().any(|t| t.name.to_lowercase() == tag)
    }

    /// Check if wallpaper has any of the given tags
    #[allow(dead_code)]
    pub fn has_any_tag(&self, tags: &[String]) -> bool {
        tags.iter().any(|t| self.has_tag(t))
    }

    /// Check if wallpaper has all of the given tags
    #[allow(dead_code)]
    pub fn has_all_tags(&self, tags: &[String]) -> bool {
        tags.iter().all(|t| self.has_tag(t))
    }

    /// Get all tags (manual + auto tag names)
    pub fn all_tags(&self) -> Vec<String> {
        let mut all: Vec<String> = self.tags.clone();
        all.extend(self.auto_tags.iter().map(|t| t.name.clone()));
        all.sort();
        all.dedup();
        all
    }

    /// Get auto tags above a confidence threshold
    #[allow(dead_code)]
    pub fn auto_tags_above(&self, threshold: f32) -> Vec<&AutoTag> {
        self.auto_tags
            .iter()
            .filter(|t| t.confidence >= threshold)
            .collect()
    }

    /// Set auto tags (replaces existing)
    #[allow(dead_code)]
    pub fn set_auto_tags(&mut self, tags: Vec<AutoTag>) {
        self.auto_tags = tags;
    }

    /// Set embedding (replaces existing)
    #[allow(dead_code)]
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(embedding);
    }

    /// Get primary/dominant color (first in list)
    #[allow(dead_code)]
    pub fn primary_color(&self) -> Option<&str> {
        self.colors.first().map(|s| s.as_str())
    }
}

impl WallpaperCache {
    fn cache_path() -> PathBuf {
        directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp"))
            .join("wallpaper_cache.json")
    }

    /// Load cached wallpapers or scan the directory if cache is invalid.
    pub fn load_or_scan(source_dir: &Path) -> Result<Self> {
        Self::load_or_scan_recursive(source_dir, false)
    }

    /// Load cache for AI operations without forcing color extraction.
    /// Falls back to metadata-only scan when no valid cache exists.
    pub fn load_or_scan_for_ai(source_dir: &Path) -> Result<Self> {
        Self::load_or_scan_for_ai_recursive(source_dir, false)
    }

    pub fn load_or_scan_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        let cache_path = Self::cache_path();

        if cache_path.exists() {
            let data = fs::read_to_string(&cache_path)?;
            if let Ok(cache) = serde_json::from_str::<WallpaperCache>(&data) {
                if cache.version != CACHE_VERSION {
                    eprintln!(
                        "Cache format changed (v{} -> v{}), rescanning...",
                        cache.version, CACHE_VERSION
                    );
                    return Self::scan_recursive(source_dir, recursive);
                }
                // Verify source dir matches and files still exist
                if cache.source_dir == source_dir && cache.validate() {
                    return Ok(cache);
                }
            }
        }

        // Scan fresh
        Self::scan_recursive(source_dir, recursive)
    }

    pub fn load_or_scan_for_ai_recursive(source_dir: &Path, recursive: bool) -> Result<Self> {
        let cache_path = Self::cache_path();

        if cache_path.exists() {
            let data = fs::read_to_string(&cache_path)?;
            if let Ok(cache) = serde_json::from_str::<WallpaperCache>(&data) {
                if cache.version != CACHE_VERSION {
                    eprintln!(
                        "Cache format changed (v{} -> v{}), rescanning...",
                        cache.version, CACHE_VERSION
                    );
                    return Self::scan_metadata_only_recursive(source_dir, recursive);
                }
                // For AI tagging we only need metadata/path validity, not extracted color palettes.
                if cache.source_dir == source_dir && cache.validate_for_ai() {
                    return Ok(cache);
                }
            }
        }

        Self::scan_metadata_only_recursive(source_dir, recursive)
    }

    pub fn scan(source_dir: &Path) -> Result<Self> {
        Self::scan_recursive(source_dir, false)
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
        if let Ok(entries) = std::fs::read_dir(&self.source_dir) {
            let current_count = entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file() && crate::utils::is_image_file(&e.path()))
                .count();

            // If file count differs significantly, invalidate
            if current_count != self.wallpapers.len() {
                return false;
            }
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
        use rand::Rng;

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

        let current = self.screen_indices.get(&screen.name).copied().unwrap_or(0);
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

    /// Get wallpapers by dominant color (hex string like "#1a2b3c")
    #[allow(dead_code)]
    pub fn with_color(&self, color: &str) -> Vec<&Wallpaper> {
        let color = color.to_lowercase();
        self.wallpapers
            .iter()
            .filter(|wp| wp.colors.iter().any(|c| c.to_lowercase() == color))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal Wallpaper for testing without filesystem access
    fn test_wallpaper(width: u32, height: u32) -> Wallpaper {
        Wallpaper {
            path: PathBuf::from("/test/fake.jpg"),
            width,
            height,
            aspect_category: Wallpaper::categorize_aspect(width, height),
            colors: vec![],
            color_weights: vec![],
            tags: vec![],
            auto_tags: vec![],
            embedding: None,
            file_size: 0,
            modified_at: 0,
        }
    }

    // --- categorize_aspect ---

    #[test]
    fn test_categorize_aspect_ultrawide() {
        assert_eq!(Wallpaper::categorize_aspect(3840, 1080), AspectCategory::Ultrawide); // 3.56
        assert_eq!(Wallpaper::categorize_aspect(5120, 1440), AspectCategory::Ultrawide); // 3.56
        assert_eq!(Wallpaper::categorize_aspect(2560, 1080), AspectCategory::Ultrawide); // 2.37
    }

    #[test]
    fn test_categorize_aspect_landscape() {
        assert_eq!(Wallpaper::categorize_aspect(1920, 1080), AspectCategory::Landscape); // 1.78
        assert_eq!(Wallpaper::categorize_aspect(1920, 1200), AspectCategory::Landscape); // 1.6
        assert_eq!(Wallpaper::categorize_aspect(2560, 1440), AspectCategory::Landscape); // 1.78
    }

    #[test]
    fn test_categorize_aspect_portrait() {
        assert_eq!(Wallpaper::categorize_aspect(1080, 1920), AspectCategory::Portrait);
        assert_eq!(Wallpaper::categorize_aspect(1440, 2560), AspectCategory::Portrait);
    }

    #[test]
    fn test_categorize_aspect_square() {
        assert_eq!(Wallpaper::categorize_aspect(1000, 1000), AspectCategory::Square);
        assert_eq!(Wallpaper::categorize_aspect(1100, 1000), AspectCategory::Square); // 1.1 < 1.2
    }

    #[test]
    fn test_categorize_aspect_boundary_landscape_square() {
        // Exactly 1.2 ratio should be Landscape
        assert_eq!(Wallpaper::categorize_aspect(1200, 1000), AspectCategory::Landscape);
        // Just below 1.2 should be Square
        assert_eq!(Wallpaper::categorize_aspect(1199, 1000), AspectCategory::Square);
    }

    #[test]
    fn test_categorize_aspect_boundary_ultrawide() {
        // Exactly 2.0 ratio should be Ultrawide
        assert_eq!(Wallpaper::categorize_aspect(2000, 1000), AspectCategory::Ultrawide);
        // Just below 2.0 should be Landscape
        assert_eq!(Wallpaper::categorize_aspect(1999, 1000), AspectCategory::Landscape);
    }

    // --- matches_screen ---

    #[test]
    fn test_matches_screen_exact() {
        let wp = test_wallpaper(1920, 1080); // Landscape
        let screen = Screen::new("DP-1".into(), 1920, 1080);
        assert!(wp.matches_screen(&screen));
    }

    #[test]
    fn test_matches_screen_different_category() {
        let wp = test_wallpaper(1920, 1080); // Landscape
        let screen = Screen::new("DP-1".into(), 1080, 1920); // Portrait
        assert!(!wp.matches_screen(&screen));
    }

    // --- matches_screen_flexible ---

    #[test]
    fn test_matches_screen_flexible_landscape_on_ultrawide() {
        let wp = test_wallpaper(1920, 1080); // Landscape
        let screen = Screen::new("DP-1".into(), 5120, 1440); // Ultrawide
        assert!(wp.matches_screen_flexible(&screen));
    }

    #[test]
    fn test_matches_screen_flexible_ultrawide_on_landscape() {
        let wp = test_wallpaper(5120, 1440); // Ultrawide
        let screen = Screen::new("DP-1".into(), 1920, 1080); // Landscape
        assert!(wp.matches_screen_flexible(&screen));
    }

    #[test]
    fn test_matches_screen_flexible_square_versatile() {
        let wp = test_wallpaper(1000, 1000); // Square
        let landscape = Screen::new("DP-1".into(), 1920, 1080);
        let portrait = Screen::new("DP-2".into(), 1080, 1920);
        let ultrawide = Screen::new("DP-3".into(), 5120, 1440);
        assert!(wp.matches_screen_flexible(&landscape));
        assert!(wp.matches_screen_flexible(&portrait));
        assert!(wp.matches_screen_flexible(&ultrawide));
    }

    #[test]
    fn test_matches_screen_flexible_portrait_not_landscape() {
        let wp = test_wallpaper(1080, 1920); // Portrait
        let screen = Screen::new("DP-1".into(), 1920, 1080); // Landscape
        assert!(!wp.matches_screen_flexible(&screen));
    }

    // --- matches_screen_with_mode ---

    #[test]
    fn test_matches_screen_with_mode_all() {
        let wp = test_wallpaper(1080, 1920); // Portrait
        let screen = Screen::new("DP-1".into(), 1920, 1080); // Landscape
        assert!(wp.matches_screen_with_mode(&screen, MatchMode::All));
    }

    // --- add_tag / remove_tag / has_tag ---

    #[test]
    fn test_add_tag() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        assert!(wp.has_tag("nature"));
        assert_eq!(wp.tags.len(), 1);
    }

    #[test]
    fn test_add_tag_duplicate() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("nature");
        assert_eq!(wp.tags.len(), 1, "Duplicate tag should not be added");
    }

    #[test]
    fn test_add_tag_case_insensitive() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("Nature");
        assert!(wp.has_tag("nature"));
        assert!(wp.has_tag("NATURE"));
    }

    #[test]
    fn test_add_tag_empty() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("");
        wp.add_tag("   ");
        assert_eq!(wp.tags.len(), 0, "Empty/whitespace tags should not be added");
    }

    #[test]
    fn test_remove_tag() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("forest");
        wp.remove_tag("nature");
        assert!(!wp.has_tag("nature"));
        assert!(wp.has_tag("forest"));
    }

    #[test]
    fn test_has_tag_includes_auto_tags() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.auto_tags.push(AutoTag {
            name: "sunset".into(),
            confidence: 0.9,
        });
        assert!(wp.has_tag("sunset"));
        assert!(wp.has_tag("SUNSET"));
    }

    // --- has_any_tag / has_all_tags ---

    #[test]
    fn test_has_any_tag() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("landscape");
        assert!(wp.has_any_tag(&["nature".into(), "ocean".into()]));
        assert!(!wp.has_any_tag(&["space".into(), "cyberpunk".into()]));
    }

    #[test]
    fn test_has_all_tags() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("landscape");
        assert!(wp.has_all_tags(&["nature".into(), "landscape".into()]));
        assert!(!wp.has_all_tags(&["nature".into(), "ocean".into()]));
    }

    // --- all_tags ---

    #[test]
    fn test_all_tags_combines_manual_and_auto() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.auto_tags.push(AutoTag {
            name: "forest".into(),
            confidence: 0.8,
        });
        let all = wp.all_tags();
        assert!(all.contains(&"nature".into()));
        assert!(all.contains(&"forest".into()));
    }

    #[test]
    fn test_all_tags_deduplicates() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.auto_tags.push(AutoTag {
            name: "nature".into(),
            confidence: 0.8,
        });
        let all = wp.all_tags();
        assert_eq!(all.iter().filter(|t| *t == "nature").count(), 1);
    }

    // --- SortMode / MatchMode cycling ---

    #[test]
    fn test_sort_mode_cycle() {
        assert_eq!(SortMode::Name.next(), SortMode::Size);
        assert_eq!(SortMode::Size.next(), SortMode::Date);
        assert_eq!(SortMode::Date.next(), SortMode::Name);
    }

    #[test]
    fn test_match_mode_cycle() {
        assert_eq!(MatchMode::Strict.next(), MatchMode::Flexible);
        assert_eq!(MatchMode::Flexible.next(), MatchMode::All);
        assert_eq!(MatchMode::All.next(), MatchMode::Strict);
    }

    // --- auto_tags_above ---

    #[test]
    fn test_auto_tags_above_threshold() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.auto_tags.push(AutoTag { name: "nature".into(), confidence: 0.9 });
        wp.auto_tags.push(AutoTag { name: "dark".into(), confidence: 0.3 });
        wp.auto_tags.push(AutoTag { name: "forest".into(), confidence: 0.7 });

        let above = wp.auto_tags_above(0.5);
        assert_eq!(above.len(), 2);
        assert!(above.iter().all(|t| t.confidence >= 0.5));
    }

    // --- primary_color ---

    #[test]
    fn test_primary_color() {
        let mut wp = test_wallpaper(1920, 1080);
        assert!(wp.primary_color().is_none());
        wp.colors = vec!["#FF0000".into(), "#00FF00".into()];
        assert_eq!(wp.primary_color(), Some("#FF0000"));
    }
}
