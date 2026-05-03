use super::App;
use crate::pairing::{extract_style_tags, MatchContext, PairingStyleMode};
use crate::screen::Screen;
use crate::utils::ColorHarmony;
use crate::wallpaper::Wallpaper;
use crate::wallpaper_backend;
use anyhow::Result;
use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

struct SelectedPairingData<'a> {
    wallpaper: &'a Wallpaper,
    tags: Vec<String>,
    style_tags: Vec<String>,
    embedding: Option<&'a [f32]>,
    weights: Cow<'a, [f32]>,
}

fn normalized_color_weights<'a>(wallpaper: &'a Wallpaper) -> Cow<'a, [f32]> {
    if wallpaper.color_weights.is_empty() {
        Cow::Owned(vec![
            1.0 / wallpaper.colors.len().max(1) as f32;
            wallpaper.colors.len()
        ])
    } else {
        Cow::Borrowed(wallpaper.color_weights.as_slice())
    }
}

impl App {
    fn other_screens(&self) -> impl Iterator<Item = (usize, &Screen)> {
        self.screens
            .iter()
            .enumerate()
            .filter(|(screen_idx, _)| *screen_idx != self.selection.screen_idx)
    }

    fn selected_pairing_data(&self) -> Option<SelectedPairingData<'_>> {
        let wallpaper = self.selected_wallpaper()?;
        let tags = wallpaper.all_tags();
        let style_tags = extract_style_tags(&tags);

        Some(SelectedPairingData {
            wallpaper,
            tags,
            style_tags,
            embedding: wallpaper.embedding.as_deref(),
            weights: normalized_color_weights(wallpaper),
        })
    }

    fn matching_wallpapers_for_screen<'a>(&'a self, screen: &'a Screen) -> Vec<&'a Wallpaper> {
        let match_mode = self.config.display.match_mode;
        self.cache
            .wallpapers
            .iter()
            .filter(|wallpaper| wallpaper.matches_screen_with_mode(screen, match_mode))
            .collect()
    }

    fn pairing_match_context<'a>(
        &'a self,
        selected: &'a SelectedPairingData<'a>,
        target_screen: &'a str,
        style_mode: PairingStyleMode,
    ) -> MatchContext<'a> {
        MatchContext {
            selected_wp: &selected.wallpaper.path,
            target_screen,
            selected_colors: &selected.wallpaper.colors,
            selected_weights: selected.weights.as_ref(),
            selected_tags: &selected.tags,
            selected_embedding: selected.embedding,
            screen_context_weight: self.config.pairing.screen_context_weight,
            visual_weight: self.config.pairing.visual_weight,
            harmony_weight: self.config.pairing.harmony_weight,
            tag_weight: self.config.pairing.tag_weight,
            semantic_weight: self.config.pairing.semantic_weight,
            repetition_penalty_weight: self.config.pairing.repetition_penalty_weight,
            style_mode,
            selected_style_tags: &selected.style_tags,
        }
    }

    fn candidate_harmony(
        &self,
        selected: &SelectedPairingData<'_>,
        candidate: &Wallpaper,
    ) -> ColorHarmony {
        let candidate_weights = normalized_color_weights(candidate);
        let (harmony, _strength) = crate::utils::detect_harmony(
            &selected.wallpaper.colors,
            selected.weights.as_ref(),
            &candidate.colors,
            candidate_weights.as_ref(),
        );
        harmony
    }

    fn preview_match_count(&self) -> usize {
        self.pairing
            .preview_matches
            .values()
            .map(|matches| matches.len())
            .max()
            .unwrap_or(0)
    }

    /// Mark pairing suggestions for background/debounced refresh.
    pub fn schedule_pairing_suggestions_update(&mut self) {
        if !self.config.pairing.enabled {
            self.pairing.suggestions.clear();
            self.pairing.suggestions_dirty = false;
            self.pairing.suggestions_due_at = None;
            return;
        }

        self.pairing.suggestions_dirty = true;
        self.pairing.suggestions_due_at =
            Some(std::time::Instant::now() + super::PAIRING_SUGGESTION_DEBOUNCE);
    }

    /// Refresh suggestions immediately.
    pub fn force_pairing_suggestions_update(&mut self) {
        self.update_pairing_suggestions();
        self.pairing.suggestions_dirty = false;
        self.pairing.suggestions_due_at = None;
    }

    /// Refresh suggestions when debounce interval has elapsed.
    /// Returns true when suggestions were recomputed.
    pub fn update_pairing_suggestions_if_due(&mut self) -> bool {
        if !self.pairing.suggestions_dirty {
            return false;
        }

        let due = self
            .pairing
            .suggestions_due_at
            .map(|when| std::time::Instant::now() >= when)
            .unwrap_or(true);

        if !due {
            return false;
        }

        self.force_pairing_suggestions_update();
        true
    }

    /// Update pairing suggestions based on currently selected wallpaper.
    pub fn update_pairing_suggestions(&mut self) {
        if !self.config.pairing.enabled {
            self.pairing.suggestions.clear();
            return;
        }

        let suggestions = {
            let Some(selected) = self.selected_pairing_data() else {
                self.pairing.suggestions.clear();
                return;
            };

            let mut suggestions = Vec::new();
            let mut seen = HashSet::new();

            for (_, screen) in self.other_screens() {
                let matching = self.matching_wallpapers_for_screen(screen);
                let match_context =
                    self.pairing_match_context(&selected, &screen.name, PairingStyleMode::Off);
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
    /// Preserves the user's current selection across the cycle when the same
    /// (screen, path) combination still appears in the new candidate list,
    /// so toggling Off → Soft → Strict doesn't yank the focus back to slot 0
    /// every time.
    pub fn toggle_pairing_style_mode(&mut self) {
        let previous_paths = self.current_preview_paths();
        self.pairing.style_mode = self.pairing.style_mode.next();
        if self.pairing.show_preview {
            self.update_pairing_preview_matches();
            self.pairing.preview_idx = self
                .find_preview_idx_matching_paths(&previous_paths)
                .unwrap_or(0);
        }
    }

    fn current_preview_paths(&self) -> HashMap<String, PathBuf> {
        self.pairing
            .preview_matches
            .iter()
            .filter_map(|(screen, matches)| {
                let idx = self
                    .pairing
                    .preview_idx
                    .min(matches.len().saturating_sub(1));
                matches
                    .get(idx)
                    .map(|(path, _, _)| (screen.clone(), path.clone()))
            })
            .collect()
    }

    fn find_preview_idx_matching_paths(&self, target: &HashMap<String, PathBuf>) -> Option<usize> {
        if target.is_empty() {
            return None;
        }
        let max = self.preview_match_count();
        (0..max).find(|&idx| {
            target.iter().all(|(screen, path)| {
                self.pairing
                    .preview_matches
                    .get(screen)
                    .and_then(|matches| {
                        let i = idx.min(matches.len().saturating_sub(1));
                        matches.get(i).map(|(p, _, _)| p == path)
                    })
                    .unwrap_or(false)
            })
        })
    }

    /// Update pairing preview matches for all other screens.
    fn update_pairing_preview_matches(&mut self) {
        if !self.config.pairing.enabled || self.screens.len() <= 1 {
            self.pairing.preview_matches.clear();
            return;
        }

        let preview_matches = {
            let Some(selected) = self.selected_pairing_data() else {
                self.pairing.preview_matches.clear();
                return;
            };

            let preview_limit = self.config.pairing.preview_match_limit.clamp(1, 50);
            let wallpaper_by_path: HashMap<&Path, &Wallpaper> = self
                .cache
                .wallpapers
                .iter()
                .map(|wp| (wp.path.as_path(), wp))
                .collect();

            let mut preview_matches = HashMap::new();

            for (_, screen) in self.other_screens() {
                let matching = self.matching_wallpapers_for_screen(screen);
                let match_context =
                    self.pairing_match_context(&selected, &screen.name, self.pairing.style_mode);
                let top_matches =
                    self.pairing
                        .history
                        .get_top_matches(&match_context, &matching, preview_limit);

                // Calculate harmony for each match.
                let matches_with_harmony: Vec<(PathBuf, f32, ColorHarmony)> = top_matches
                    .into_iter()
                    .map(|(path, score)| {
                        let harmony = wallpaper_by_path
                            .get(path.as_path())
                            .map(|wp| self.candidate_harmony(&selected, wp))
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
        let max_alternatives = self.preview_match_count();

        if max_alternatives > 0 {
            self.pairing.preview_idx = (self.pairing.preview_idx + 1) % max_alternatives;
        }
    }

    /// Cycle through pairing preview alternatives backwards.
    pub fn pairing_preview_prev(&mut self) {
        let max_alternatives = self.preview_match_count();

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

        let previous_wallpapers = self.pairing.current_wallpapers.clone();

        // First apply the selected wallpaper to current screen.
        self.apply_wallpaper()?;

        // Then apply the preview selections to other screens.
        for (screen_name, matches) in &self.pairing.preview_matches {
            let idx = self
                .pairing
                .preview_idx
                .min(matches.len().saturating_sub(1));
            if let Some((wp_path, _, _)) = matches.get(idx) {
                if let Err(e) = wallpaper_backend::set_wallpaper_with_resize(
                    &self.config.backend,
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

        if previous_wallpapers != self.pairing.current_wallpapers {
            self.pairing.history.arm_undo(
                previous_wallpapers,
                self.config.pairing.undo_window_secs,
                "Pairing applied",
            );
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
        self.preview_match_count()
    }
}
