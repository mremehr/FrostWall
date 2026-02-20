use crate::swww::{FillColor, ResizeMode, Transition, TransitionType};
use crate::wallpaper::MatchMode;
use anyhow::Result;
use crossterm::event::KeyCode;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub wallpaper: WallpaperConfig,
    #[serde(default)]
    pub display: DisplayConfig,
    #[serde(default)]
    pub transition: TransitionConfig,
    #[serde(default)]
    pub thumbnails: ThumbnailConfig,
    #[serde(default)]
    pub theme: ThemeConfig,
    #[serde(default)]
    pub keybindings: KeybindingsConfig,
    #[serde(default)]
    pub clip: ClipConfig,
    #[serde(default)]
    pub pairing: PairingConfig,
    #[serde(default)]
    pub time_profiles: crate::timeprofile::TimeProfiles,
    #[serde(default)]
    pub terminal: TerminalConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperConfig {
    pub directory: PathBuf,
    pub extensions: Vec<String>,
    pub recursive: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayConfig {
    #[serde(default)]
    pub match_mode: MatchMode,
    #[serde(default)]
    pub resize_mode: ResizeMode,
    #[serde(default)]
    pub fill_color: FillColor,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionConfig {
    pub transition_type: String,
    pub duration: f32,
    pub fps: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThumbnailConfig {
    pub width: u32,
    pub height: u32,
    pub quality: u8,
    pub grid_columns: usize,
    #[serde(default = "default_preload_count")]
    pub preload_count: usize,
}

fn default_preload_count() -> usize {
    3
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub mode: String, // "auto", "light", "dark"
    pub check_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TerminalConfig {
    /// Recommended repaint_delay for kitty.conf (ms)
    #[serde(default = "default_repaint_delay")]
    pub recommended_repaint_delay: u32,
    /// Recommended input_delay for kitty.conf (ms)
    #[serde(default = "default_input_delay")]
    pub recommended_input_delay: u32,
    /// Whether the optimization hint has been shown
    #[serde(default)]
    pub hint_shown: bool,
}

fn default_repaint_delay() -> u32 {
    5
}

fn default_input_delay() -> u32 {
    1
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            recommended_repaint_delay: 5,
            recommended_input_delay: 1,
            hint_shown: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeybindingsConfig {
    pub next: String,
    pub prev: String,
    pub apply: String,
    pub quit: String,
    pub random: String,
    pub toggle_match: String,
    pub toggle_resize: String,
    pub next_screen: String,
    pub prev_screen: String,
}

/// Configuration for CLIP auto-tagging
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipConfig {
    /// Enable CLIP auto-tagging during scans
    pub enabled: bool,
    /// Confidence threshold for tags (0.0-1.0)
    pub threshold: f32,
    /// Include auto-tags in tag filter UI
    pub show_in_filter: bool,
    /// Cache embeddings for similarity search
    pub cache_embeddings: bool,
}

/// Configuration for intelligent wallpaper pairing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingConfig {
    /// Enable intelligent pairing suggestions
    pub enabled: bool,
    /// Auto-apply suggestions to other screens
    pub auto_apply: bool,
    /// Show undo option duration (seconds)
    pub undo_window_secs: u64,
    /// Minimum confidence to auto-apply (0.0-1.0)
    pub auto_apply_threshold: f32,
    /// Maximum history records to keep
    pub max_history_records: usize,
    /// Number of candidate matches shown in pairing preview
    #[serde(default = "default_pairing_preview_match_limit")]
    pub preview_match_limit: usize,
    /// Weight for screen-specific co-occurrence history
    #[serde(default = "default_pairing_screen_context_weight")]
    pub screen_context_weight: f32,
    /// Weight for visual similarity (palette/brightness/saturation)
    #[serde(default = "default_pairing_visual_weight")]
    pub visual_weight: f32,
    /// Weight for color harmony bonus
    #[serde(default = "default_pairing_harmony_weight")]
    pub harmony_weight: f32,
    /// Weight for shared tag bonus
    #[serde(default = "default_pairing_tag_weight")]
    pub tag_weight: f32,
    /// Weight for semantic CLIP embedding similarity
    #[serde(default = "default_pairing_semantic_weight")]
    pub semantic_weight: f32,
    /// Multiplier for recent repetition penalty
    #[serde(default = "default_pairing_repetition_penalty_weight")]
    pub repetition_penalty_weight: f32,
}

fn default_pairing_preview_match_limit() -> usize {
    10
}

fn default_pairing_screen_context_weight() -> f32 {
    8.0
}

fn default_pairing_visual_weight() -> f32 {
    5.0
}

fn default_pairing_harmony_weight() -> f32 {
    3.0
}

fn default_pairing_tag_weight() -> f32 {
    2.0
}

fn default_pairing_semantic_weight() -> f32 {
    7.0
}

fn default_pairing_repetition_penalty_weight() -> f32 {
    1.0
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            directory: dirs::picture_dir()
                .map(|p| p.join("wallpapers"))
                .unwrap_or_else(|| PathBuf::from("~/Pictures/wallpapers")),
            extensions: vec![
                "jpg".into(),
                "jpeg".into(),
                "png".into(),
                "webp".into(),
                "bmp".into(),
                "gif".into(),
            ],
            recursive: false,
        }
    }
}

impl Default for DisplayConfig {
    fn default() -> Self {
        Self {
            match_mode: MatchMode::Flexible,
            resize_mode: ResizeMode::Fit,
            fill_color: FillColor::black(),
        }
    }
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            transition_type: "fade".to_string(),
            duration: 1.0,
            fps: 60,
        }
    }
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            width: 800,
            height: 600,
            quality: 92,
            grid_columns: 3,
            preload_count: 3,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: "auto".to_string(),
            check_interval_ms: 500,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next: "l".to_string(),
            prev: "h".to_string(),
            apply: "Enter".to_string(),
            quit: "q".to_string(),
            random: "r".to_string(),
            toggle_match: "m".to_string(),
            toggle_resize: "f".to_string(),
            next_screen: "Tab".to_string(),
            prev_screen: "BackTab".to_string(),
        }
    }
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            enabled: false, // Opt-in by default
            threshold: 0.25,
            show_in_filter: true,
            cache_embeddings: true,
        }
    }
}

impl Default for PairingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_apply: false, // Conservative default
            undo_window_secs: 5,
            auto_apply_threshold: 0.7,
            max_history_records: 1000,
            preview_match_limit: default_pairing_preview_match_limit(),
            screen_context_weight: default_pairing_screen_context_weight(),
            visual_weight: default_pairing_visual_weight(),
            harmony_weight: default_pairing_harmony_weight(),
            tag_weight: default_pairing_tag_weight(),
            semantic_weight: default_pairing_semantic_weight(),
            repetition_penalty_weight: default_pairing_repetition_penalty_weight(),
        }
    }
}

impl KeybindingsConfig {
    /// Parse a keybinding string into a KeyCode
    pub fn parse_key(s: &str) -> Option<KeyCode> {
        let s = s.trim();

        // Single character
        if s.len() == 1 {
            if let Some(ch) = s.chars().next() {
                return Some(KeyCode::Char(ch));
            }
        }

        // Named keys (case insensitive)
        match s.to_lowercase().as_str() {
            "enter" | "return" => Some(KeyCode::Enter),
            "esc" | "escape" => Some(KeyCode::Esc),
            "tab" => Some(KeyCode::Tab),
            "backtab" | "shift+tab" | "s-tab" => Some(KeyCode::BackTab),
            "space" => Some(KeyCode::Char(' ')),
            "backspace" => Some(KeyCode::Backspace),
            "delete" | "del" => Some(KeyCode::Delete),
            "insert" | "ins" => Some(KeyCode::Insert),
            "home" => Some(KeyCode::Home),
            "end" => Some(KeyCode::End),
            "pageup" | "pgup" => Some(KeyCode::PageUp),
            "pagedown" | "pgdn" => Some(KeyCode::PageDown),
            "up" | "arrow_up" => Some(KeyCode::Up),
            "down" | "arrow_down" => Some(KeyCode::Down),
            "left" | "arrow_left" => Some(KeyCode::Left),
            "right" | "arrow_right" => Some(KeyCode::Right),
            "f1" => Some(KeyCode::F(1)),
            "f2" => Some(KeyCode::F(2)),
            "f3" => Some(KeyCode::F(3)),
            "f4" => Some(KeyCode::F(4)),
            "f5" => Some(KeyCode::F(5)),
            "f6" => Some(KeyCode::F(6)),
            "f7" => Some(KeyCode::F(7)),
            "f8" => Some(KeyCode::F(8)),
            "f9" => Some(KeyCode::F(9)),
            "f10" => Some(KeyCode::F(10)),
            "f11" => Some(KeyCode::F(11)),
            "f12" => Some(KeyCode::F(12)),
            _ => None,
        }
    }

    /// Check if a KeyCode matches a keybinding
    pub fn matches(&self, key: KeyCode, binding: &str) -> bool {
        Self::parse_key(binding) == Some(key)
    }
}

impl Config {
    /// Return the path to the configuration file.
    pub fn config_path() -> PathBuf {
        directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.config_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."))
            .join("config.toml")
    }

    /// Load config from file, creating default if missing or corrupt.
    pub fn load() -> Result<Self> {
        let path = Self::config_path();

        if path.exists() {
            let data = fs::read_to_string(&path)?;
            match toml::from_str::<Config>(&data) {
                Ok(config) => Ok(config),
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to parse config at {}: {}",
                        path.display(),
                        e
                    );
                    eprintln!("Using default configuration.");
                    let config = Config::default();
                    config.save()?;
                    Ok(config)
                }
            }
        } else {
            // Create default config.
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

    /// Check if running in Kitty terminal.
    pub fn is_kitty_terminal() -> bool {
        std::env::var("TERM")
            .map(|t| t.contains("kitty"))
            .unwrap_or(false)
            || std::env::var("KITTY_WINDOW_ID").is_ok()
    }

    /// Show terminal optimization hint if not shown before.
    /// Returns the hint message if it should be shown.
    pub fn check_terminal_hint(&mut self) -> Option<String> {
        if self.terminal.hint_shown || !Self::is_kitty_terminal() {
            return None;
        }

        self.terminal.hint_shown = true;
        let _ = self.save(); // Save that hint was shown.

        Some(format!(
            "Tip: För optimal prestanda i Kitty, lägg till i ~/.config/kitty/kitty.conf:\n\n\
             repaint_delay {}\n\
             input_delay {}\n\
             sync_to_monitor yes\n\n\
             Tryck valfri tangent för att fortsätta...",
            self.terminal.recommended_repaint_delay, self.terminal.recommended_input_delay
        ))
    }

    /// Build a Transition struct from config settings.
    pub fn transition(&self) -> Transition {
        let transition_type = match self.transition.transition_type.as_str() {
            "fade" => TransitionType::Fade,
            "wipe" => TransitionType::Wipe,
            "grow" => TransitionType::Grow,
            "center" => TransitionType::Center,
            "outer" => TransitionType::Outer,
            "none" => TransitionType::None,
            _ => TransitionType::Fade,
        };

        Transition {
            transition_type,
            duration: self.transition.duration,
            fps: self.transition.fps,
        }
    }

    /// Get wallpaper directory, expanding ~ if needed.
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
