use crate::wallpaper::Wallpaper;
use anyhow::{Context, Result};
use std::path::Path;

fn read_file_metadata(path: &Path) -> (u64, u64) {
    let metadata = std::fs::metadata(path).ok();
    let file_size = metadata
        .as_ref()
        .map(|metadata| metadata.len())
        .unwrap_or(0);
    let modified_at = metadata
        .and_then(|metadata| metadata.modified().ok())
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_secs())
        .unwrap_or(0);
    (file_size, modified_at)
}

impl Wallpaper {
    /// Fast path: only read dimensions from image header (no full decode).
    pub fn from_path_fast(path: &Path) -> Result<Self> {
        let (width, height) =
            image::image_dimensions(path).context("Failed to read image dimensions")?;
        let aspect_category = Self::categorize_aspect(width, height);
        let (file_size, modified_at) = read_file_metadata(path);

        Ok(Self {
            path: path.to_path_buf(),
            width,
            height,
            aspect_category,
            colors: Vec::new(),
            color_weights: Vec::new(),
            tags: Vec::new(),
            auto_tags: Vec::new(),
            embedding: None,
            file_size,
            modified_at,
        })
    }

    /// Full path with colors (legacy, slower).
    pub fn from_path(path: &Path) -> Result<Self> {
        let mut wallpaper = Self::from_path_fast(path)?;
        wallpaper.extract_colors()?;
        Ok(wallpaper)
    }
}
