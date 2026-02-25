use super::App;
use crate::swww;
use anyhow::Result;

impl App {
    /// Apply the selected wallpaper to the current screen via swww.
    pub fn apply_wallpaper(&mut self) -> Result<()> {
        let (screen_name, wp_path, pywal_error) = {
            let (Some(screen), Some(wp)) = (self.selected_screen(), self.selected_wallpaper())
            else {
                return Ok(());
            };

            swww::set_wallpaper_with_resize(
                &screen.name,
                &wp.path,
                &self.config.transition(),
                self.config.display.resize_mode,
                &self.config.display.fill_color,
            )?;

            let pywal_error = if self.ui.pywal_export {
                crate::pywal::generate_from_wallpaper(&wp.colors, &wp.path)
                    .err()
                    .map(|e| format!("pywal: {}", e))
            } else {
                None
            };

            (screen.name.clone(), wp.path.clone(), pywal_error)
        };

        // Update current wallpaper for this screen.
        self.pairing.current_wallpapers.insert(screen_name, wp_path);

        if let Some(error) = pywal_error {
            self.ui.status_message = Some(error);
        }
        Ok(())
    }

    /// Handle undo action (restore previous wallpapers).
    pub fn do_undo(&mut self) -> Result<()> {
        if let Some(previous) = self.pairing.history.do_undo() {
            for (screen_name, wp_path) in &previous {
                swww::set_wallpaper_with_resize(
                    screen_name,
                    wp_path,
                    &self.config.transition(),
                    self.config.display.resize_mode,
                    &self.config.display.fill_color,
                )?;
            }
            // Restore current_wallpapers tracking.
            self.pairing.current_wallpapers = previous;
        }
        Ok(())
    }

    /// Check and clear expired undo window.
    pub fn tick_undo(&mut self) {
        self.pairing.history.clear_expired_undo();
    }

    /// Pick a random wallpaper from the filtered list and apply it.
    pub fn random_wallpaper(&mut self) -> Result<()> {
        if !self.selection.filtered_wallpapers.is_empty() {
            use rand::Rng;
            let mut rng = rand::thread_rng();
            self.selection.wallpaper_idx =
                rng.gen_range(0..self.selection.filtered_wallpapers.len());

            // Apply immediately.
            self.apply_wallpaper()?;
        }
        Ok(())
    }

    /// Incremental rescan: find new files, remove deleted ones, keep all
    /// existing tags, auto-tags, CLIP embeddings and color data intact.
    /// Returns a human-readable status message.
    pub fn rescan(&mut self) -> Result<String> {
        let recursive = self.config.wallpaper.recursive;
        let (added, removed) = self.cache.incremental_rescan(recursive)?;
        self.update_filtered_wallpapers();

        let total = self.cache.wallpapers.len();
        let mut parts = vec![format!("{} wallpapers", total)];
        if added > 0 {
            parts.push(format!("+{} new", added));
        }
        if removed > 0 {
            parts.push(format!("-{} removed", removed));
        }
        if added == 0 && removed == 0 {
            parts.push("no changes".to_string());
        }

        let needs_tagging = self
            .cache
            .wallpapers
            .iter()
            .filter(|wp| wp.auto_tags.is_empty())
            .count();
        if needs_tagging > 0 && self.config.clip.enabled {
            parts.push(format!("{} untagged", needs_tagging));
        }

        Ok(parts.join(", "))
    }

    /// Toggle help popup.
    pub fn toggle_help(&mut self) {
        self.ui.show_help = !self.ui.show_help;
    }

    /// Export pywal colors for current wallpaper.
    pub fn export_pywal(&self) -> Result<()> {
        if let Some(wp) = self.selected_wallpaper() {
            crate::pywal::generate_from_wallpaper(&wp.colors, &wp.path)?;
        }
        Ok(())
    }

    /// Toggle pywal export on apply.
    pub fn toggle_pywal_export(&mut self) {
        self.ui.pywal_export = !self.ui.pywal_export;
    }
}
