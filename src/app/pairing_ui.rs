use super::App;
use crate::pairing::{extract_style_tags, MatchContext, PairingStyleMode};
use crate::swww;
use crate::utils::ColorHarmony;
use crate::wallpaper::Wallpaper;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

impl App {
    /// Update pairing suggestions based on currently selected wallpaper.
    pub fn update_pairing_suggestions(&mut self) {
        self.pairing.suggestions.clear();

        if !self.config.pairing.enabled {
            return;
        }

        // Get selected wallpaper context (colors + AI/manual tags).
        let (selected_path, selected_colors, selected_weights, selected_tags, selected_embedding) =
            match self.selected_wallpaper() {
                Some(wp) => (
                    wp.path.clone(),
                    wp.colors.clone(),
                    wp.color_weights.clone(),
                    wp.all_tags(),
                    wp.embedding.clone(),
                ),
                None => return,
            };
        let selected_style_tags = extract_style_tags(&selected_tags);

        // Get suggestions from pairing history.
        let match_mode = self.config.display.match_mode;

        // For each other screen, find suggested wallpapers.
        for (screen_idx, screen) in self.screens.iter().enumerate() {
            if screen_idx == self.selection.screen_idx {
                continue;
            }

            // Get wallpapers that match this screen.
            let matching: Vec<_> = self
                .cache
                .wallpapers
                .iter()
                .filter(|wp| wp.matches_screen_with_mode(screen, match_mode))
                .collect();

            // Find best match based on pairing history + color similarity.
            let match_context = MatchContext {
                selected_wp: &selected_path,
                target_screen: &screen.name,
                selected_colors: &selected_colors,
                selected_weights: &selected_weights,
                selected_tags: &selected_tags,
                selected_embedding: selected_embedding.as_deref(),
                screen_context_weight: self.config.pairing.screen_context_weight,
                visual_weight: self.config.pairing.visual_weight,
                harmony_weight: self.config.pairing.harmony_weight,
                tag_weight: self.config.pairing.tag_weight,
                semantic_weight: self.config.pairing.semantic_weight,
                repetition_penalty_weight: self.config.pairing.repetition_penalty_weight,
                style_mode: PairingStyleMode::Off,
                selected_style_tags: &selected_style_tags,
            };
            if let Some(suggested_path) = self
                .pairing
                .history
                .get_best_match(&match_context, &matching)
            {
                if !self.pairing.suggestions.contains(&suggested_path) {
                    self.pairing.suggestions.push(suggested_path);
                }
            }
        }
    }

    /// Check if a wallpaper is in the pairing suggestions.
    pub fn is_pairing_suggestion(&self, path: &Path) -> bool {
        self.pairing.suggestions.iter().any(|p| p == path)
    }

    /// Toggle pairing preview popup.
    pub fn toggle_pairing_preview(&mut self) {
        if !self.pairing.show_preview {
            self.update_pairing_preview_matches();
        }
        self.pairing.show_preview = !self.pairing.show_preview;
        self.pairing.preview_idx = 0;
    }

    /// Cycle style matching behavior used in pairing preview.
    pub fn toggle_pairing_style_mode(&mut self) {
        self.pairing.style_mode = self.pairing.style_mode.next();
        if self.pairing.show_preview {
            self.update_pairing_preview_matches();
            self.pairing.preview_idx = 0;
        }
    }

    /// Update pairing preview matches for all other screens.
    fn update_pairing_preview_matches(&mut self) {
        self.pairing.preview_matches.clear();

        if !self.config.pairing.enabled || self.screens.len() <= 1 {
            return;
        }

        let (selected_path, selected_colors, selected_weights, selected_tags, selected_embedding) =
            match self.selected_wallpaper() {
                Some(wp) => (
                    wp.path.clone(),
                    wp.colors.clone(),
                    wp.color_weights.clone(),
                    wp.all_tags(),
                    wp.embedding.clone(),
                ),
                None => return,
            };
        let selected_style_tags = extract_style_tags(&selected_tags);

        // Default weights if empty.
        let selected_weights = if selected_weights.is_empty() {
            vec![1.0 / selected_colors.len().max(1) as f32; selected_colors.len()]
        } else {
            selected_weights
        };

        let match_mode = self.config.display.match_mode;
        let preview_limit = self.config.pairing.preview_match_limit.clamp(1, 50);
        let wallpaper_by_path: HashMap<&Path, &Wallpaper> = self
            .cache
            .wallpapers
            .iter()
            .map(|wp| (wp.path.as_path(), wp))
            .collect();

        for (screen_idx, screen) in self.screens.iter().enumerate() {
            if screen_idx == self.selection.screen_idx {
                continue;
            }

            // Get wallpapers that match this screen.
            let matching: Vec<_> = self
                .cache
                .wallpapers
                .iter()
                .filter(|wp| wp.matches_screen_with_mode(screen, match_mode))
                .collect();

            // Get top pairing matches for preview.
            let match_context = MatchContext {
                selected_wp: &selected_path,
                target_screen: &screen.name,
                selected_colors: &selected_colors,
                selected_weights: &selected_weights,
                selected_tags: &selected_tags,
                selected_embedding: selected_embedding.as_deref(),
                screen_context_weight: self.config.pairing.screen_context_weight,
                visual_weight: self.config.pairing.visual_weight,
                harmony_weight: self.config.pairing.harmony_weight,
                tag_weight: self.config.pairing.tag_weight,
                semantic_weight: self.config.pairing.semantic_weight,
                repetition_penalty_weight: self.config.pairing.repetition_penalty_weight,
                style_mode: self.pairing.style_mode,
                selected_style_tags: &selected_style_tags,
            };
            let top_matches =
                self.pairing
                    .history
                    .get_top_matches(&match_context, &matching, preview_limit);

            // Calculate harmony for each match.
            let matches_with_harmony: Vec<(PathBuf, f32, ColorHarmony)> = top_matches
                .into_iter()
                .map(|(path, score)| {
                    // Find the wallpaper to get its colors and weights.
                    let harmony = wallpaper_by_path
                        .get(path.as_path())
                        .map(|wp| {
                            let wp_weights = if wp.color_weights.is_empty() {
                                vec![1.0 / wp.colors.len().max(1) as f32; wp.colors.len()]
                            } else {
                                wp.color_weights.clone()
                            };
                            let (harmony, _strength) = crate::utils::detect_harmony(
                                &selected_colors,
                                &selected_weights,
                                &wp.colors,
                                &wp_weights,
                            );
                            harmony
                        })
                        .unwrap_or(ColorHarmony::None);
                    (path, score, harmony)
                })
                .collect();

            if !matches_with_harmony.is_empty() {
                self.pairing
                    .preview_matches
                    .insert(screen.name.clone(), matches_with_harmony);
            }
        }
    }

    /// Cycle through pairing preview alternatives.
    pub fn pairing_preview_next(&mut self) {
        let max_alternatives = self
            .pairing
            .preview_matches
            .values()
            .map(|v| v.len())
            .max()
            .unwrap_or(1);

        if max_alternatives > 0 {
            self.pairing.preview_idx = (self.pairing.preview_idx + 1) % max_alternatives;
        }
    }

    /// Cycle through pairing preview alternatives backwards.
    pub fn pairing_preview_prev(&mut self) {
        let max_alternatives = self
            .pairing
            .preview_matches
            .values()
            .map(|v| v.len())
            .max()
            .unwrap_or(1);

        if max_alternatives > 0 {
            self.pairing.preview_idx = if self.pairing.preview_idx == 0 {
                max_alternatives - 1
            } else {
                self.pairing.preview_idx - 1
            };
        }
    }

    /// Apply the currently selected pairing preview.
    pub fn apply_pairing_preview(&mut self) -> Result<()> {
        if !self.pairing.show_preview {
            return Ok(());
        }

        // First apply the selected wallpaper to current screen.
        self.apply_wallpaper()?;

        // Then apply the preview selections to other screens.
        for (screen_name, matches) in &self.pairing.preview_matches {
            let idx = self
                .pairing
                .preview_idx
                .min(matches.len().saturating_sub(1));
            if let Some((wp_path, _, _)) = matches.get(idx) {
                if let Err(e) = swww::set_wallpaper_with_resize(
                    screen_name,
                    wp_path,
                    &self.config.transition(),
                    self.config.display.resize_mode,
                    &self.config.display.fill_color,
                ) {
                    self.ui.status_message = Some(format!("Pairing {}: {}", screen_name, e));
                } else {
                    self.pairing
                        .current_wallpapers
                        .insert(screen_name.clone(), wp_path.clone());
                }
            }
        }

        // Record the pairing.
        if self.pairing.current_wallpapers.len() > 1 {
            self.pairing
                .history
                .record_pairing(self.pairing.current_wallpapers.clone(), true);
        }

        self.pairing.show_preview = false;
        Ok(())
    }

    /// Get the number of alternatives available in pairing preview.
    pub fn pairing_preview_alternatives(&self) -> usize {
        self.pairing
            .preview_matches
            .values()
            .map(|v| v.len())
            .max()
            .unwrap_or(0)
    }
}
