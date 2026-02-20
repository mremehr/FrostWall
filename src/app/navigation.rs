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
}
