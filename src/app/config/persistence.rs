use super::sections::Config;
use crate::utils::project_config_dir;
use crate::wallpaper_backend::Transition;
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

impl Config {
    /// Return the path to the configuration file.
    pub fn config_path() -> PathBuf {
        project_config_dir(PathBuf::from(".")).join("config.toml")
    }

    /// Load config from file, creating default if missing or corrupt.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let data = fs::read_to_string(&path)?;
            match toml::from_str::<Config>(&data) {
                Ok(config) => Ok(config),
                Err(error) => {
                    eprintln!(
                        "Warning: Failed to parse config at {}: {}",
                        path.display(),
                        error
                    );
                    eprintln!("Using default configuration.");
                    let config = Config::default();
                    config.save()?;
                    Ok(config)
                }
            }
        } else {
            let config = Config::default();
            config.save()?;
            Ok(config)
        }
    }

    /// Save config to file.
    pub fn save(&self) -> Result<()> {
        let path = Self::config_path();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let data = toml::to_string_pretty(self)?;
        fs::write(&path, data)?;
        Ok(())
    }

    /// Rewrite persisted TUI session paths after wallpaper files are renamed.
    pub fn remap_session_paths(&mut self, mapping: &HashMap<PathBuf, PathBuf>) -> Result<usize> {
        let mut updated = 0;

        if let Some(path) = self.session.last_selected_wallpaper.as_mut() {
            if let Some(new_path) = mapping.get(path) {
                *path = new_path.clone();
                updated += 1;
            }
        }

        for path in self.session.last_selected_wallpaper_by_screen.values_mut() {
            if let Some(new_path) = mapping.get(path) {
                *path = new_path.clone();
                updated += 1;
            }
        }

        if updated > 0 {
            self.save()?;
        }

        Ok(updated)
    }

    /// Check if running in Kitty terminal.
    pub fn is_kitty_terminal() -> bool {
        std::env::var("TERM")
            .map(|term| term.contains("kitty"))
            .unwrap_or(false)
            || std::env::var("KITTY_WINDOW_ID").is_ok()
    }

    /// Whether to force the safe thumbnail protocol path on Kitty.
    /// Env override: `FROSTWALL_KITTY_SAFE_THUMBNAILS=0|1`.
    pub fn use_safe_kitty_thumbnail_protocol(&self) -> bool {
        match std::env::var("FROSTWALL_KITTY_SAFE_THUMBNAILS")
            .ok()
            .map(|value| value.trim().to_ascii_lowercase())
            .as_deref()
        {
            Some("1" | "true" | "yes" | "on") => true,
            Some("0" | "false" | "no" | "off") => false,
            _ => self.terminal.kitty_safe_thumbnails,
        }
    }

    /// Show terminal optimization hint if not shown before.
    /// Returns the hint message if it should be shown.
    pub fn check_terminal_hint(&mut self) -> Option<String> {
        if self.terminal.hint_shown || !Self::is_kitty_terminal() {
            return None;
        }

        self.terminal.hint_shown = true;
        let _ = self.save();

        Some(format!(
            "Tip: For optimal performance in Kitty, add this to ~/.config/kitty/kitty.conf:\n\n\
             repaint_delay {}\n\
             input_delay {}\n\
             sync_to_monitor yes\n\n\
             Press any key to continue...",
            self.terminal.recommended_repaint_delay, self.terminal.recommended_input_delay
        ))
    }

    /// Build a Transition struct from config settings.
    pub fn transition(&self) -> Transition {
        Transition {
            transition_type: self.transition.transition_type,
            duration: self.transition.duration,
            fps: self.transition.fps,
        }
    }

    /// Get wallpaper directory, expanding `~` if needed.
    pub fn wallpaper_dir(&self) -> PathBuf {
        let dir = &self.wallpaper.directory;
        if dir.starts_with("~") {
            if let Some(home) = dirs::home_dir() {
                return home.join(dir.strip_prefix("~").unwrap_or(dir));
            }
        }
        dir.clone()
    }
}
