use super::App;
use crate::wallpaper::SortMode;

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

            // Image protocol mode (HB/KTY).
            "img" | "image" => {
                let mode = args.to_ascii_lowercase();
                match mode.as_str() {
                    "" | "toggle" => self.toggle_thumbnail_protocol_mode(),
                    "hb" | "safe" | "halfblocks" => {
                        self.config.terminal.kitty_safe_thumbnails = true;
                        self.reset_thumbnail_cache();
                        self.thumbnails.image_picker =
                            Some(Self::new_thumbnail_picker(&self.config));
                        self.ui.status_message = Some("Thumbnail protocol: HB (safe)".to_string());
                    }
                    "kty" | "kitty" => {
                        self.config.terminal.kitty_safe_thumbnails = false;
                        self.reset_thumbnail_cache();
                        self.thumbnails.image_picker =
                            Some(Self::new_thumbnail_picker(&self.config));
                        self.ui.status_message = Some("Thumbnail protocol: KTY".to_string());
                    }
                    _ => {
                        self.ui.status_message = Some("Usage: :img [toggle|hb|kitty]".to_string());
                    }
                }
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
                self.find_and_select_similar();
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
    fn find_and_select_similar(&mut self) {
        let next_pos = {
            let Some(current_cache_idx) = self
                .selection
                .filtered_wallpapers
                .get(self.selection.wallpaper_idx)
                .copied()
            else {
                return;
            };
            let Some(current_wp) = self.cache.wallpapers.get(current_cache_idx) else {
                return;
            };

            let wallpaper_colors: Vec<(usize, &[String])> = self
                .cache
                .wallpapers
                .iter()
                .enumerate()
                .filter(|(idx, wp)| *idx != current_cache_idx && !wp.colors.is_empty())
                .map(|(idx, wp)| (idx, wp.colors.as_slice()))
                .collect();

            let similar =
                crate::utils::find_similar_wallpapers(&current_wp.colors, &wallpaper_colors, 1);
            similar.first().and_then(|(_, idx)| {
                self.selection
                    .filtered_wallpapers
                    .iter()
                    .position(|&cache_idx| cache_idx == *idx)
            })
        };

        if let Some(pos) = next_pos {
            self.selection.wallpaper_idx = pos;
        }
    }
}
