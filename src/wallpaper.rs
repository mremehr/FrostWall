use crate::clip::AutoTag;
use crate::screen::AspectCategory;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

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

#[derive(Debug, Clone, Copy)]
pub enum CacheLoadMode {
    Full,
    MetadataOnly,
}

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
    /// Whether the cache was built with recursive scanning
    #[serde(default)]
    pub recursive: bool,
    /// Cached list of matching wallpaper indices per screen key.
    #[serde(skip)]
    pub screen_match_indices: HashMap<String, Vec<usize>>,
    /// Precomputed palette features per wallpaper index for fast similarity search.
    #[serde(skip)]
    pub similarity_profiles: Vec<crate::utils::PaletteProfile>,
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub total: usize,
    pub ultrawide: usize,
    pub landscape: usize,
    pub portrait: usize,
    pub square: usize,
}

mod cache;
mod model;

#[cfg(test)]
mod tests;
