use super::App;
use crate::wallpaper::SortMode;

#[derive(Debug, PartialEq, Eq)]
enum Command<'a> {
    Quit,
    Tag(&'a str),
    Clear,
    Random,
    Apply,
    Image(&'a str),
    Sort(&'a str),
    Similar,
    Rescan,
    Help,
    Screen(&'a str),
    Go(&'a str),
    PairRebuild,
    Unknown(String),
}

fn parse_command(input: &str) -> Option<Command<'_>> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }

    let mut words = trimmed.split_whitespace();
    let name = words.next()?;
    let command = name.to_ascii_lowercase();
    let args = trimmed[name.len()..].trim();

    Some(match command.as_str() {
        "q" | "quit" | "exit" => Command::Quit,
        "t" | "tag" => Command::Tag(args),
        "c" | "clear" => Command::Clear,
        "r" | "random" => Command::Random,
        "a" | "apply" => Command::Apply,
        "img" | "image" => Command::Image(args),
        "sort" => Command::Sort(args),
        "similar" | "sim" => Command::Similar,
        "rescan" | "scan" => Command::Rescan,
        "h" | "help" => Command::Help,
        "screen" => Command::Screen(args),
        "go" | "g" => Command::Go(args),
        "pair-reset" | "pair-rebuild" => Command::PairRebuild,
        _ => Command::Unknown(command),
    })
}

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

        let Some(command) = parse_command(&cmd) else {
            return;
        };

        match command {
            Command::Quit => {
                self.ui.should_quit = true;
            }

            Command::Tag(args) => {
                if args.is_empty() {
                    let tags = self.cache.all_tags();
                    if tags.is_empty() {
                        self.ui.status_message = Some("No tags available".to_string());
                    } else {
                        self.ui.status_message = Some(format!("Tags: {}", tags.join(", ")));
                    }
                } else {
                    let tag = args.to_string();
                    let tags = self.cache.all_tags();
                    let args_lower = args.to_ascii_lowercase();
                    if let Some(matched) = tags
                        .into_iter()
                        .find(|t| t.to_ascii_lowercase().contains(&args_lower))
                    {
                        self.filters.active_tag = Some(matched);
                        self.update_filtered_wallpapers();
                    } else {
                        self.ui.status_message = Some(format!("Tag not found: {}", tag));
                    }
                }
            }

            Command::Clear => {
                self.filters.active_tag = None;
                self.filters.active_color = None;
                self.update_filtered_wallpapers();
            }

            Command::Random => {
                let _ = self.random_wallpaper();
            }

            Command::Apply => {
                let _ = self.apply_wallpaper();
            }

            Command::Image(args) => {
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

            Command::Sort(args) => match args.to_ascii_lowercase().as_str() {
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

            Command::Similar => {
                self.find_and_select_similar();
            }

            Command::Rescan => match self.rescan() {
                Ok(msg) => {
                    self.ui.status_message = Some(format!("Rescan: {}", msg));
                }
                Err(e) => {
                    self.ui.status_message = Some(format!("Rescan: {}", e));
                }
            },

            Command::Help => {
                self.ui.show_help = true;
            }

            Command::Screen(args) => {
                if let Ok(n) = args.parse::<usize>() {
                    if n > 0 && n <= self.screens.len() {
                        self.selection
                            .screen_positions
                            .insert(self.selection.screen_idx, self.selection.wallpaper_idx);
                        self.selection.screen_idx = n - 1;
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
                    } else {
                        self.ui.status_message = Some(format!("Screen {} not found", n));
                    }
                }
            }

            Command::Go(args) => {
                if let Ok(n) = args.parse::<usize>() {
                    if n > 0 && n <= self.selection.filtered_wallpapers.len() {
                        self.selection.wallpaper_idx = n - 1;
                    }
                }
            }

            Command::PairRebuild => {
                let records = self.pairing.history.record_count();
                self.pairing.history.rebuild_affinity();
                self.ui.status_message = Some(format!(
                    "Rebuilt affinity from {} records ({} pairs)",
                    records,
                    self.pairing.history.affinity_count()
                ));
            }

            Command::Unknown(command) => {
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

#[cfg(test)]
mod tests {
    use super::{parse_command, Command};

    #[test]
    fn parse_known_aliases() {
        assert_eq!(parse_command("q"), Some(Command::Quit));
        assert_eq!(parse_command("QUIT"), Some(Command::Quit));
        assert_eq!(parse_command("pair-reset"), Some(Command::PairRebuild));
        assert_eq!(parse_command("pair-rebuild"), Some(Command::PairRebuild));
        assert_eq!(parse_command("sim"), Some(Command::Similar));
    }

    #[test]
    fn parse_arguments_are_trimmed() {
        assert_eq!(
            parse_command("tag   nature  "),
            Some(Command::Tag("nature"))
        );
        assert_eq!(parse_command("screen    2"), Some(Command::Screen("2")));
        assert_eq!(parse_command("go\t10"), Some(Command::Go("10")));
    }

    #[test]
    fn parse_unknown_command_is_lowercased() {
        assert_eq!(
            parse_command("FoObAr arg"),
            Some(Command::Unknown("foobar".to_string()))
        );
    }

    #[test]
    fn parse_empty_command_returns_none() {
        assert_eq!(parse_command("   "), None);
        assert_eq!(parse_command(""), None);
    }
}
