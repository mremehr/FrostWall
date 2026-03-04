use super::App;
use crate::screen::AspectCategory;
use crate::wallpaper::SortMode;
use std::cmp::Ordering;

fn aspect_sort_rank(aspect: AspectCategory) -> u8 {
    match aspect {
        AspectCategory::Ultrawide => 0,
        AspectCategory::Landscape => 1,
        AspectCategory::Square => 2,
        AspectCategory::Portrait => 3,
    }
}

impl App {
    /// Recompute filtered list + sort. Does NOT reset thumbnail cache or schedule
    /// pairing suggestions. Use when updating multiple screens sequentially.
    pub(super) fn update_filtered_wallpapers_core(&mut self) {
        let match_mode = self.config.display.match_mode;
        let tag_filter = self.filters.active_tag.as_deref();
        let color_filter = self.filters.active_color.as_deref();

        if let Some(screen) = self.screens.get(self.selection.screen_idx) {
            self.selection.filtered_wallpapers = self
                .cache
                .wallpapers
                .iter()
                .enumerate()
                .filter(|(_, wp)| {
                    // Screen matching.
                    if !wp.matches_screen_with_mode(screen, match_mode) {
                        return false;
                    }
                    // Tag filtering.
                    if let Some(tag) = tag_filter {
                        if !wp.has_tag(tag) {
                            return false;
                        }
                    }
                    // Color filtering with perceptual matching.
                    if let Some(color) = color_filter {
                        // Include if any color is perceptually similar (>0.7 similarity).
                        let has_similar = wp
                            .colors
                            .iter()
                            .any(|c| crate::utils::color_similarity(c, color) > 0.7);
                        if !has_similar {
                            return false;
                        }
                    }
                    true
                })
                .map(|(i, _)| i)
                .collect();
        } else {
            self.selection.filtered_wallpapers = (0..self.cache.wallpapers.len()).collect();
        }

        // Apply current sort.
        self.apply_sort();

        if self.selection.filtered_wallpapers.is_empty() {
            self.selection.wallpaper_idx = 0;
        } else if self.selection.wallpaper_idx >= self.selection.filtered_wallpapers.len() {
            self.selection.wallpaper_idx = self.selection.filtered_wallpapers.len() - 1;
        }
    }

    /// Recompute the filtered wallpaper list based on screen, tag, and color filters.
    /// Full update: filter, sort, reset thumbnail cache, schedule suggestions.
    pub fn update_filtered_wallpapers(&mut self) {
        self.update_filtered_wallpapers_core();
        self.filters.available_colors = self.get_unique_colors();
        // Clear thumbnail state after filter changes so IDs don't drift/wrap.
        self.reset_thumbnail_cache();
        self.schedule_pairing_suggestions_update();
    }

    /// Toggle match mode and refresh filter.
    pub fn toggle_match_mode(&mut self) {
        self.config.display.match_mode = self.config.display.match_mode.next();
        self.update_filtered_wallpapers();
    }

    /// Toggle resize mode.
    pub fn toggle_resize_mode(&mut self) {
        self.config.display.resize_mode = self.config.display.resize_mode.next();
    }

    /// Cycle through sort modes.
    pub fn toggle_sort_mode(&mut self) {
        self.filters.sort_mode = self.filters.sort_mode.next();
        self.apply_sort();
    }

    /// Toggle grouping by aspect category.
    pub fn toggle_aspect_sort(&mut self) {
        self.filters.aspect_sort_enabled = !self.filters.aspect_sort_enabled;
        self.config.display.aspect_sort = self.filters.aspect_sort_enabled;
        self.apply_sort();
        self.ui.status_message = Some(format!(
            "Aspect sort: {} (ultrawide→landscape→square→portrait)",
            if self.filters.aspect_sort_enabled {
                "ON"
            } else {
                "OFF"
            }
        ));
    }

    /// Apply current sort mode to filtered wallpapers.
    fn apply_sort(&mut self) {
        let cache = &self.cache;
        let sort_mode = self.filters.sort_mode;
        let aspect_sort_enabled = self.filters.aspect_sort_enabled;

        self.selection.filtered_wallpapers.sort_by(|&a, &b| {
            let wp_a = &cache.wallpapers[a];
            let wp_b = &cache.wallpapers[b];

            if aspect_sort_enabled {
                let aspect_order = aspect_sort_rank(wp_a.aspect_category)
                    .cmp(&aspect_sort_rank(wp_b.aspect_category));
                if aspect_order != Ordering::Equal {
                    return aspect_order;
                }
            }

            let primary = match sort_mode {
                SortMode::Name => wp_a.path.cmp(&wp_b.path),
                SortMode::Size => {
                    // Use cached file_size (no filesystem calls).
                    wp_b.file_size.cmp(&wp_a.file_size) // Largest first.
                }
                SortMode::Date => {
                    // Use cached modified_at (no filesystem calls).
                    wp_b.modified_at.cmp(&wp_a.modified_at) // Newest first.
                }
            };

            primary.then_with(|| wp_a.path.cmp(&wp_b.path))
        });

        // Reset selection after sort.
        self.selection.wallpaper_idx = 0;
    }

    /// Toggle color display for selected wallpaper.
    pub fn toggle_colors(&mut self) {
        self.ui.show_colors = !self.ui.show_colors;
    }

    /// Cycle through available tags as filter.
    pub fn cycle_tag_filter(&mut self) {
        let all_tags = self.cache.all_tags();

        if all_tags.is_empty() {
            self.filters.active_tag = None;
            self.ui.status_message = Some(
                "No tags defined. Use 'frostwall tag add <path> <tag>' to add tags.".to_string(),
            );
            return;
        }

        self.filters.active_tag = match &self.filters.active_tag {
            None => Some(all_tags[0].clone()),
            Some(current) => {
                // Find current position and move to next.
                if let Some(pos) = all_tags.iter().position(|t| t == current) {
                    if pos + 1 < all_tags.len() {
                        Some(all_tags[pos + 1].clone())
                    } else {
                        None // Wrap around to "all".
                    }
                } else {
                    None
                }
            }
        };

        // Clear any previous error.
        self.ui.status_message = None;
        self.update_filtered_wallpapers();
    }

    /// Clear tag filter.
    pub fn clear_tag_filter(&mut self) {
        self.filters.active_tag = None;
        self.update_filtered_wallpapers();
    }

    /// Toggle color picker popup.
    pub fn toggle_color_picker(&mut self) {
        self.filters.color_picker_idx = 0;
        self.ui.show_color_picker = !self.ui.show_color_picker;
    }

    /// Get unique colors across all wallpapers.
    fn get_unique_colors(&self) -> Vec<String> {
        let mut colors: Vec<String> = self
            .cache
            .wallpapers
            .iter()
            .flat_map(|wp| wp.colors.iter().cloned())
            .collect();
        colors.sort();
        colors.dedup();
        // Limit to reasonable number.
        colors.truncate(32);
        colors
    }

    /// Navigate color picker.
    pub fn color_picker_next(&mut self) {
        if !self.filters.available_colors.is_empty() {
            self.filters.color_picker_idx =
                (self.filters.color_picker_idx + 1) % self.filters.available_colors.len();
        }
    }

    /// Navigate color picker backwards.
    pub fn color_picker_prev(&mut self) {
        if !self.filters.available_colors.is_empty() {
            self.filters.color_picker_idx = if self.filters.color_picker_idx == 0 {
                self.filters.available_colors.len() - 1
            } else {
                self.filters.color_picker_idx - 1
            };
        }
    }

    /// Apply selected color filter.
    pub fn apply_color_filter(&mut self) {
        if let Some(color) = self
            .filters
            .available_colors
            .get(self.filters.color_picker_idx)
        {
            self.filters.active_color = Some(color.clone());
            self.ui.show_color_picker = false;
            self.update_filtered_wallpapers();
        }
    }

    /// Clear color filter.
    pub fn clear_color_filter(&mut self) {
        self.filters.active_color = None;
        self.update_filtered_wallpapers();
    }
}
