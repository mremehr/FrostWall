use super::{AnalysisFailure, AnalysisRequest, AnalysisResponse, App};
use crate::utils::build_palette_profile;
use std::sync::mpsc::SyncSender;

impl App {
    pub fn request_color_analysis(&mut self, cache_idx: usize) {
        if cache_idx >= self.cache.wallpapers.len() {
            return;
        }

        let Some(wallpaper) = self.cache.wallpapers.get(cache_idx) else {
            return;
        };
        if !wallpaper.colors.is_empty() || self.analysis.loading.contains(&cache_idx) {
            return;
        }

        let Some(tx) = &self.analysis.request_tx else {
            return;
        };

        let request = AnalysisRequest {
            cache_idx,
            source_path: wallpaper.path.clone(),
            generation: self.analysis.generation,
        };

        if tx.try_send(request).is_ok() {
            self.analysis.loading.insert(cache_idx);
        }
    }

    pub fn set_analysis_channel(&mut self, tx: SyncSender<AnalysisRequest>) {
        self.analysis.request_tx = Some(tx);
    }

    pub(super) fn reset_analysis_state(&mut self) {
        self.analysis.loading.clear();
        self.analysis.generation = self.analysis.generation.wrapping_add(1);
    }

    pub fn handle_analysis_ready(&mut self, response: AnalysisResponse) {
        if response.generation != self.analysis.generation {
            return;
        }

        self.analysis.loading.remove(&response.cache_idx);

        let Some(wallpaper) = self.cache.wallpapers.get_mut(response.cache_idx) else {
            return;
        };
        wallpaper.colors = response.colors;
        wallpaper.color_weights = response.color_weights;

        let profile = build_palette_profile(&wallpaper.colors, &wallpaper.color_weights);
        if self.cache.similarity_profiles.len() <= response.cache_idx {
            self.cache
                .similarity_profiles
                .resize(response.cache_idx + 1, Default::default());
        }
        self.cache.similarity_profiles[response.cache_idx] = profile;

        let mut colors_changed = false;
        for color in &wallpaper.colors {
            if !self
                .filters
                .available_colors
                .iter()
                .any(|existing| existing == color)
            {
                self.filters.available_colors.push(color.clone());
                colors_changed = true;
            }
        }
        if colors_changed {
            self.filters.available_colors.sort();
            self.filters.available_colors.dedup();
            self.filters.available_colors.truncate(32);
        }

        let selected_cache_idx = self
            .selection
            .filtered_wallpapers
            .get(self.selection.wallpaper_idx)
            .copied();
        if selected_cache_idx == Some(response.cache_idx) {
            self.schedule_pairing_suggestions_update();
        }
    }

    pub fn handle_analysis_failed(&mut self, failure: AnalysisFailure) {
        if failure.generation != self.analysis.generation {
            return;
        }

        self.analysis.loading.remove(&failure.cache_idx);

        let selected_cache_idx = self
            .selection
            .filtered_wallpapers
            .get(self.selection.wallpaper_idx)
            .copied();
        if selected_cache_idx == Some(failure.cache_idx) {
            self.ui.status_message = Some("Failed to analyze colors".to_string());
        }
    }
}
