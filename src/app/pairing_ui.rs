use super::App;
use crate::pairing::{extract_style_tags, MatchContext, PairingStyleMode};
use crate::swww;
use crate::utils::ColorHarmony;
use crate::wallpaper::Wallpaper;
use anyhow::Result;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

impl App {
    /// Update pairing suggestions based on currently selected wallpaper.
    pub fn update_pairing_suggestions(&mut self) {
        if !self.config.pairing.enabled {
            self.pairing.suggestions.clear();
            return;
        }

        let suggestions = {
            let Some(selected_wp) = self.selected_wallpaper() else {
                self.pairing.suggestions.clear();
                return;
            };

            let selected_tags = selected_wp.all_tags();
            let selected_style_tags = extract_style_tags(&selected_tags);
            let selected_embedding = selected_wp.embedding.as_deref();
            let match_mode = self.config.display.match_mode;

            let mut suggestions = Vec::new();
            let mut seen = HashSet::new();

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
                    selected_wp: &selected_wp.path,
                    target_screen: &screen.name,
                    selected_colors: &selected_wp.colors,
                    selected_weights: &selected_wp.color_weights,
                    selected_tags: &selected_tags,
                    selected_embedding,
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
                    if seen.insert(suggested_path.clone()) {
                        suggestions.push(suggested_path);
                    }
                }
            }

            suggestions
        };

        self.pairing.suggestions = suggestions;
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
        if !self.config.pairing.enabled || self.screens.len() <= 1 {
            self.pairing.preview_matches.clear();
            return;
        }

        let preview_matches = {
            let Some(selected_wp) = self.selected_wallpaper() else {
                self.pairing.preview_matches.clear();
                return;
            };

            let selected_tags = selected_wp.all_tags();
            let selected_style_tags = extract_style_tags(&selected_tags);
            let selected_embedding = selected_wp.embedding.as_deref();
            let selected_colors = &selected_wp.colors;

            // Default weights if empty.
            let selected_default_weights;
            let selected_weights: &[f32] = if selected_wp.color_weights.is_empty() {
                selected_default_weights =
                    vec![1.0 / selected_colors.len().max(1) as f32; selected_colors.len()];
                &selected_default_weights
            } else {
                selected_wp.color_weights.as_slice()
            };

            let match_mode = self.config.display.match_mode;
            let preview_limit = self.config.pairing.preview_match_limit.clamp(1, 50);
            let wallpaper_by_path: HashMap<&Path, &Wallpaper> = self
                .cache
                .wallpapers
                .iter()
                .map(|wp| (wp.path.as_path(), wp))
                .collect();

            let mut preview_matches = HashMap::new();

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
                    selected_wp: &selected_wp.path,
                    target_screen: &screen.name,
                    selected_colors,
                    selected_weights,
                    selected_tags: &selected_tags,
                    selected_embedding,
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
                                let wp_default_weights;
                                let wp_weights: &[f32] = if wp.color_weights.is_empty() {
                                    wp_default_weights =
                                        vec![1.0 / wp.colors.len().max(1) as f32; wp.colors.len()];
                                    &wp_default_weights
                                } else {
                                    wp.color_weights.as_slice()
                                };
                                let (harmony, _strength) = crate::utils::detect_harmony(
                                    selected_colors,
                                    selected_weights,
                                    &wp.colors,
                                    wp_weights,
                                );
                                harmony
                            })
                            .unwrap_or(ColorHarmony::None);
                        (path, score, harmony)
                    })
                    .collect();

                if !matches_with_harmony.is_empty() {
                    preview_matches.insert(screen.name.clone(), matches_with_harmony);
                }
            }

            preview_matches
        };

        self.pairing.preview_matches = preview_matches;
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
