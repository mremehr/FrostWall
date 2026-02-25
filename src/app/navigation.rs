use super::App;
use crate::screen::Screen;
use crate::wallpaper::Wallpaper;

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

    /// Restore the previously selected wallpaper from persisted session state.
    pub fn restore_last_selection(&mut self) {
        let Some(saved_path) = self.config.session.last_selected_wallpaper.clone() else {
            return;
        };

        if self.screens.is_empty() {
            return;
        }

        let original_screen_idx = self
            .selection
            .screen_idx
            .min(self.screens.len().saturating_sub(1));

        for screen_idx in 0..self.screens.len() {
            self.selection.screen_idx = screen_idx;
            self.update_filtered_wallpapers();

            if let Some(pos) = self
                .selection
                .filtered_wallpapers
                .iter()
                .position(|&cache_idx| {
                    self.cache
                        .wallpapers
                        .get(cache_idx)
                        .map(|wp| wp.path == saved_path)
                        .unwrap_or(false)
                })
            {
                self.selection.wallpaper_idx = pos;
                return;
            }
        }

        self.selection.screen_idx = original_screen_idx;
        self.update_filtered_wallpapers();
    }

    /// Persist the currently selected wallpaper so selection survives restart.
    pub fn persist_last_selection(&mut self) {
        self.config.session.last_selected_wallpaper =
            self.selected_wallpaper().map(|wp| wp.path.clone());
    }
}
