mod matching;
mod persistence;
mod scanning;
mod tags;

use super::{CacheStats, WallpaperCache};
use crate::screen::AspectCategory;
use crate::utils::project_cache_dir;
use std::path::PathBuf;

impl WallpaperCache {
    pub(super) fn cache_path() -> PathBuf {
        project_cache_dir(PathBuf::from("/tmp")).join("wallpaper_cache.json")
    }

    pub(super) fn startup_cache_path() -> PathBuf {
        project_cache_dir(PathBuf::from("/tmp")).join("wallpaper_startup_cache.json")
    }

    pub(super) fn rebuild_similarity_profiles(&mut self) {
        self.similarity_profiles = self
            .wallpapers
            .iter()
            .map(|wp| crate::utils::build_palette_profile(&wp.colors, &wp.color_weights))
            .collect();
    }

    pub fn ensure_similarity_profiles(&mut self) {
        let needs_rebuild = self.similarity_profiles.len() != self.wallpapers.len()
            || self
                .wallpapers
                .iter()
                .zip(self.similarity_profiles.iter())
                .any(|(wp, profile)| profile.normalized_weights.len() != wp.colors.len());

        if needs_rebuild {
            self.rebuild_similarity_profiles();
        }
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
}
