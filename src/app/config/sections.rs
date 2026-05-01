use super::defaults::{
    default_input_delay, default_kitty_safe_thumbnails, default_pairing_harmony_weight,
    default_pairing_preview_match_limit, default_pairing_repetition_penalty_weight,
    default_pairing_screen_context_weight, default_pairing_semantic_weight,
    default_pairing_tag_weight, default_pairing_visual_weight, default_preload_count,
    default_repaint_delay,
};
use crate::wallpaper::MatchMode;
use crate::wallpaper_backend::{BackendConfig, FillColor, ResizeMode, TransitionType};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub backend: BackendConfig,
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
    #[serde(default)]
    pub session: SessionConfig,
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
    /// Persisted aspect grouping for carousel ordering.
    #[serde(default)]
    pub aspect_sort: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TransitionConfig {
    #[serde(default)]
    pub transition_type: TransitionType,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ThemeConfig {
    pub mode: String,
    pub check_interval_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SessionConfig {
    /// Legacy global selection used by older versions.
    #[serde(default)]
    pub last_selected_wallpaper: Option<PathBuf>,
    /// Persist selection per monitor name so each screen restores its own cursor.
    #[serde(default)]
    pub last_selected_wallpaper_by_screen: HashMap<String, PathBuf>,
    /// Persist which monitor was active when the TUI closed.
    #[serde(default)]
    pub last_active_screen: Option<String>,
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
    /// Force safe thumbnail protocol on Kitty (uses halfblocks instead of Kitty graphics).
    #[serde(default = "default_kitty_safe_thumbnails")]
    pub kitty_safe_thumbnails: bool,
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

/// Configuration for CLIP auto-tagging.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClipConfig {
    /// Enable CLIP auto-tagging during scans
    pub enabled: bool,
    /// Confidence threshold for tags (0.0-1.0)
    pub threshold: f32,
    /// Number of images to send per CLIP inference pass
    #[serde(default = "super::defaults::default_clip_batch_size")]
    pub batch_size: usize,
    /// Include auto-tags in tag filter UI
    pub show_in_filter: bool,
    /// Cache embeddings for similarity search
    pub cache_embeddings: bool,
    /// Optional override URL for the CLIP visual ONNX model
    #[serde(default)]
    pub visual_model_url: Option<String>,
    /// Optional SHA256 for custom visual model URL
    #[serde(default)]
    pub visual_model_sha256: Option<String>,
}

/// Configuration for intelligent wallpaper pairing.
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
