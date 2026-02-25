use super::{MatchMode, Wallpaper};
#[cfg(any(test, feature = "clip"))]
use crate::clip::AutoTag;
use crate::screen::{AspectCategory, Screen};
use anyhow::{Context, Result};
use image::imageops::FilterType;
use kmeans_colors::get_kmeans_hamerly;
use palette::{IntoColor, Lab, Srgb};
use std::path::Path;

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
        let thumb_rgb = thumb.to_rgb8();
        let mut lab: Vec<Lab> =
            Vec::with_capacity((thumb_rgb.width() * thumb_rgb.height()) as usize);
        lab.extend(thumb_rgb.pixels().map(|p| {
            let rgb = Srgb::new(
                p.0[0] as f32 / 255.0,
                p.0[1] as f32 / 255.0,
                p.0[2] as f32 / 255.0,
            );
            let lab_color: Lab = rgb.into_color();
            lab_color
        }));

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
        if width == 0 || height == 0 {
            return AspectCategory::Square;
        }
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
    #[cfg(test)]
    pub fn has_any_tag(&self, tags: &[String]) -> bool {
        tags.iter().any(|t| self.has_tag(t))
    }

    /// Check if wallpaper has all of the given tags
    #[cfg(test)]
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
    #[cfg(test)]
    pub fn auto_tags_above(&self, threshold: f32) -> Vec<&AutoTag> {
        self.auto_tags
            .iter()
            .filter(|t| t.confidence >= threshold)
            .collect()
    }

    /// Set auto tags (replaces existing)
    #[cfg(feature = "clip")]
    pub fn set_auto_tags(&mut self, tags: Vec<AutoTag>) {
        self.auto_tags = tags;
    }

    /// Set embedding (replaces existing)
    #[cfg(feature = "clip")]
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(embedding);
    }

    /// Get primary/dominant color (first in list)
    #[cfg(test)]
    pub fn primary_color(&self) -> Option<&str> {
        self.colors.first().map(|s| s.as_str())
    }
}
