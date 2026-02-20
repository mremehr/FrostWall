use super::App;
use crate::wallpaper::SortMode;
use std::path::Path;

impl App {
    /// Enter command mode.
    pub fn enter_command_mode(&mut self) {
        self.ui.command_mode = true;
        self.ui.command_buffer.clear();
    }

    /// Exit command mode without executing.
    pub fn exit_command_mode(&mut self) {
        self.ui.command_mode = false;
        self.ui.command_buffer.clear();
    }

    /// Add character to command buffer.
    pub fn command_input(&mut self, c: char) {
        self.ui.command_buffer.push(c);
    }

    /// Remove last character from command buffer.
    pub fn command_backspace(&mut self) {
        self.ui.command_buffer.pop();
    }

    /// Execute the current command.
    pub fn execute_command(&mut self) {
        let cmd = self.ui.command_buffer.trim().to_string();
        self.ui.command_mode = false;
        self.ui.command_buffer.clear();

        if cmd.is_empty() {
            return;
        }

        // Parse command and args.
        let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
        let command = parts[0].to_lowercase();
        let args = parts.get(1).map(|s| s.trim()).unwrap_or("");

        match command.as_str() {
            // Quit.
            "q" | "quit" | "exit" => {
                self.ui.should_quit = true;
            }

            // Tag filter.
            "t" | "tag" => {
                if args.is_empty() {
                    // List available tags.
                    let tags = self.cache.all_tags();
                    if tags.is_empty() {
                        self.ui.status_message = Some("No tags available".to_string());
                    } else {
                        self.ui.status_message = Some(format!("Tags: {}", tags.join(", ")));
                    }
                } else {
                    // Filter by tag.
                    let tag = args.to_string();
                    let tags = self.cache.all_tags();
                    // Fuzzy match - find tag that contains the search term.
                    if let Some(matched) = tags
                        .iter()
                        .find(|t| t.to_lowercase().contains(&args.to_lowercase()))
                    {
                        self.filters.active_tag = Some(matched.clone());
                        self.update_filtered_wallpapers();
                    } else {
                        self.ui.status_message = Some(format!("Tag not found: {}", tag));
                    }
                }
            }

            // Clear filters.
            "c" | "clear" => {
                self.filters.active_tag = None;
                self.filters.active_color = None;
                self.update_filtered_wallpapers();
            }

            // Random wallpaper.
            "r" | "random" => {
                let _ = self.random_wallpaper();
            }

            // Apply current wallpaper.
            "a" | "apply" => {
                let _ = self.apply_wallpaper();
            }

            // Sort mode.
            "sort" => match args.to_lowercase().as_str() {
                "name" | "n" => {
                    self.filters.sort_mode = SortMode::Name;
                    self.update_filtered_wallpapers();
                }
                "date" | "d" => {
                    self.filters.sort_mode = SortMode::Date;
                    self.update_filtered_wallpapers();
                }
                "size" | "s" => {
                    self.filters.sort_mode = SortMode::Size;
                    self.update_filtered_wallpapers();
                }
                _ => {
                    self.ui.status_message = Some("Sort modes: name, date, size".to_string());
                }
            },

            // Similar wallpapers.
            "similar" | "sim" => {
                if let Some(wp) = self.selected_wallpaper() {
                    let colors = wp.colors.clone();
                    let path = wp.path.clone();
                    self.find_and_select_similar(&colors, &path);
                }
            }

            // Rescan wallpaper directory.
            "rescan" | "scan" => match self.rescan() {
                Ok(msg) => {
                    self.ui.status_message = Some(format!("Rescan: {}", msg));
                }
                Err(e) => {
                    self.ui.status_message = Some(format!("Rescan: {}", e));
                }
            },

            // Help.
            "h" | "help" => {
                self.ui.show_help = true;
            }

            // Screen navigation.
            "screen" => {
                if let Ok(n) = args.parse::<usize>() {
                    if n > 0 && n <= self.screens.len() {
                        // Save current position.
                        self.selection
                            .screen_positions
                            .insert(self.selection.screen_idx, self.selection.wallpaper_idx);
                        self.selection.screen_idx = n - 1;
                        self.update_filtered_wallpapers();
                        // Restore position for new screen.
                        if let Some(&pos) = self
                            .selection
                            .screen_positions
                            .get(&self.selection.screen_idx)
                        {
                            if pos < self.selection.filtered_wallpapers.len() {
                                self.selection.wallpaper_idx = pos;
                            }
                        }
                    } else {
                        self.ui.status_message = Some(format!("Screen {} not found", n));
                    }
                }
            }

            // Go to wallpaper by number.
            "go" | "g" => {
                if let Ok(n) = args.parse::<usize>() {
                    if n > 0 && n <= self.selection.filtered_wallpapers.len() {
                        self.selection.wallpaper_idx = n - 1;
                    }
                }
            }

            // Rebuild pairing affinity scores from history.
            "pair-reset" | "pair-rebuild" => {
                let records = self.pairing.history.record_count();
                self.pairing.history.rebuild_affinity();
                self.ui.status_message = Some(format!(
                    "Rebuilt affinity from {} records ({} pairs)",
                    records,
                    self.pairing.history.affinity_count()
                ));
            }

            _ => {
                self.ui.status_message = Some(format!("Unknown command: {}", command));
            }
        }
    }

    /// Find similar wallpapers and select the best match.
    fn find_and_select_similar(&mut self, colors: &[String], current_path: &Path) {
        let wallpaper_colors: Vec<(usize, &[String])> = self
            .cache
            .wallpapers
            .iter()
            .enumerate()
            .filter(|(_, wp)| wp.path != current_path && !wp.colors.is_empty())
            .map(|(i, wp)| (i, wp.colors.as_slice()))
            .collect();

        let similar = crate::utils::find_similar_wallpapers(colors, &wallpaper_colors, 1);
        if let Some((_, idx)) = similar.first() {
            // Find this index in filtered wallpapers.
            if let Some(pos) = self
                .selection
                .filtered_wallpapers
                .iter()
                .position(|&i| i == *idx)
            {
                self.selection.wallpaper_idx = pos;
            }
        }
    }
}
