use crate::wallpaper::{Wallpaper, WallpaperCache};
use std::path::Path;

impl WallpaperCache {
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
