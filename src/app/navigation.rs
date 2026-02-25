use super::App;
use crate::screen::Screen;
use crate::wallpaper::Wallpaper;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

fn find_position_by_path(
    filtered_wallpapers: &[usize],
    wallpapers: &[Wallpaper],
    saved_path: &Path,
) -> Option<usize> {
    filtered_wallpapers.iter().position(|&cache_idx| {
        wallpapers
            .get(cache_idx)
            .map(|wp| wp.path == saved_path)
            .unwrap_or(false)
    })
}

impl App {
    /// Return the currently selected wallpaper, if any.
    pub fn selected_wallpaper(&self) -> Option<&Wallpaper> {
        self.selection
            .filtered_wallpapers
            .get(self.selection.wallpaper_idx)
            .and_then(|&i| self.cache.wallpapers.get(i))
    }

    /// Return the currently selected screen, if any.
    pub fn selected_screen(&self) -> Option<&Screen> {
        self.screens.get(self.selection.screen_idx)
    }

    /// Select the next wallpaper in the filtered list.
    pub fn next_wallpaper(&mut self) {
        if !self.selection.filtered_wallpapers.is_empty() {
            self.selection.wallpaper_idx =
                (self.selection.wallpaper_idx + 1) % self.selection.filtered_wallpapers.len();
            self.update_pairing_suggestions();
        }
    }

    /// Select the previous wallpaper in the filtered list.
    pub fn prev_wallpaper(&mut self) {
        if !self.selection.filtered_wallpapers.is_empty() {
            self.selection.wallpaper_idx = if self.selection.wallpaper_idx == 0 {
                self.selection.filtered_wallpapers.len() - 1
            } else {
                self.selection.wallpaper_idx - 1
            };
            self.update_pairing_suggestions();
        }
    }

    /// Switch to the next screen, preserving cursor position per screen.
    pub fn next_screen(&mut self) {
        if !self.screens.is_empty() {
            // Save current position.
            self.selection
                .screen_positions
                .insert(self.selection.screen_idx, self.selection.wallpaper_idx);

            self.selection.screen_idx = (self.selection.screen_idx + 1) % self.screens.len();
            self.update_filtered_wallpapers();

            // Restore position for new screen (if saved).
            if let Some(&pos) = self
                .selection
                .screen_positions
                .get(&self.selection.screen_idx)
            {
                if pos < self.selection.filtered_wallpapers.len() {
                    self.selection.wallpaper_idx = pos;
                }
            }

            self.update_pairing_suggestions();
        }
    }

    /// Switch to the previous screen, preserving cursor position per screen.
    pub fn prev_screen(&mut self) {
        if !self.screens.is_empty() {
            // Save current position.
            self.selection
                .screen_positions
                .insert(self.selection.screen_idx, self.selection.wallpaper_idx);

            self.selection.screen_idx = if self.selection.screen_idx == 0 {
                self.screens.len() - 1
            } else {
                self.selection.screen_idx - 1
            };
            self.update_filtered_wallpapers();

            // Restore position for new screen (if saved).
            if let Some(&pos) = self
                .selection
                .screen_positions
                .get(&self.selection.screen_idx)
            {
                if pos < self.selection.filtered_wallpapers.len() {
                    self.selection.wallpaper_idx = pos;
                }
            }

            self.update_pairing_suggestions();
        }
    }

    /// Restore persisted selection for each monitor and active screen.
    pub fn restore_last_selection(&mut self) {
        if self.screens.is_empty() {
            return;
        }

        let saved_by_screen = self
            .config
            .session
            .last_selected_wallpaper_by_screen
            .clone();
        let legacy_saved_path = self.config.session.last_selected_wallpaper.clone();
        let preferred_screen_name = self.config.session.last_active_screen.clone();

        let original_screen_idx = self
            .selection
            .screen_idx
            .min(self.screens.len().saturating_sub(1));

        self.selection.screen_positions.clear();
        let mut first_restored_screen_idx = None;

        for screen_idx in 0..self.screens.len() {
            let screen_name = self
                .screens
                .get(screen_idx)
                .map(|screen| screen.name.clone())
                .unwrap_or_default();

            let saved_path: Option<PathBuf> =
                saved_by_screen.get(&screen_name).cloned().or_else(|| {
                    if saved_by_screen.is_empty() {
                        legacy_saved_path.clone()
                    } else {
                        None
                    }
                });

            self.selection.screen_idx = screen_idx;
            self.update_filtered_wallpapers();

            if let Some(saved_path) = saved_path.as_deref() {
                if let Some(pos) = find_position_by_path(
                    &self.selection.filtered_wallpapers,
                    &self.cache.wallpapers,
                    saved_path,
                ) {
                    self.selection.screen_positions.insert(screen_idx, pos);
                    if first_restored_screen_idx.is_none() {
                        first_restored_screen_idx = Some(screen_idx);
                    }
                }
            }
        }

        self.selection.screen_idx = preferred_screen_name
            .as_deref()
            .and_then(|name| self.screens.iter().position(|screen| screen.name == name))
            .or(first_restored_screen_idx)
            .unwrap_or(original_screen_idx);

        self.update_filtered_wallpapers();

        if let Some(&pos) = self
            .selection
            .screen_positions
            .get(&self.selection.screen_idx)
        {
            if pos < self.selection.filtered_wallpapers.len() {
                self.selection.wallpaper_idx = pos;
            }
        }
    }

    /// Persist selection for every visited screen and remember active monitor.
    pub fn persist_last_selection(&mut self) {
        if self.screens.is_empty() {
            self.config.session.last_selected_wallpaper = None;
            self.config
                .session
                .last_selected_wallpaper_by_screen
                .clear();
            self.config.session.last_active_screen = None;
            return;
        }

        // Capture current screen position too, even if user exits without switching.
        self.selection
            .screen_positions
            .insert(self.selection.screen_idx, self.selection.wallpaper_idx);

        let original_screen_idx = self
            .selection
            .screen_idx
            .min(self.screens.len().saturating_sub(1));
        let original_wallpaper_idx = self.selection.wallpaper_idx;
        let original_filtered_wallpapers = self.selection.filtered_wallpapers.clone();
        let mut saved_by_screen: HashMap<String, PathBuf> = HashMap::new();

        for screen_idx in 0..self.screens.len() {
            let Some(saved_pos) = self.selection.screen_positions.get(&screen_idx).copied() else {
                continue;
            };

            self.selection.screen_idx = screen_idx;
            self.update_filtered_wallpapers();

            let Some(&cache_idx) = self.selection.filtered_wallpapers.get(saved_pos) else {
                continue;
            };
            let Some(wallpaper_path) = self
                .cache
                .wallpapers
                .get(cache_idx)
                .map(|wp| wp.path.clone())
            else {
                continue;
            };
            let Some(screen_name) = self
                .screens
                .get(screen_idx)
                .map(|screen| screen.name.clone())
            else {
                continue;
            };

            saved_by_screen.insert(screen_name, wallpaper_path);
        }

        self.config.session.last_selected_wallpaper_by_screen = saved_by_screen;
        self.config.session.last_active_screen = self
            .screens
            .get(original_screen_idx)
            .map(|screen| screen.name.clone());

        // Restore in-memory state after probing other screens.
        self.selection.screen_idx = original_screen_idx;
        self.selection.filtered_wallpapers = original_filtered_wallpapers;
        self.selection.wallpaper_idx = if self.selection.filtered_wallpapers.is_empty() {
            0
        } else {
            original_wallpaper_idx.min(self.selection.filtered_wallpapers.len() - 1)
        };

        // Keep legacy key populated for backward compatibility.
        self.config.session.last_selected_wallpaper =
            self.selected_wallpaper().map(|wp| wp.path.clone());
    }
}
