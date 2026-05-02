use super::sections::{
    ClipConfig, DisplayConfig, KeybindingsConfig, PairingConfig, TerminalConfig, ThemeConfig,
    ThumbnailConfig, TransitionConfig, WallpaperConfig,
};
use crate::wallpaper::MatchMode;
use crate::wallpaper_backend::{FillColor, ResizeMode, TransitionType};
use std::path::PathBuf;

const DEFAULT_THUMBNAIL_PRELOAD_COUNT: usize = 20;
const DEFAULT_TERMINAL_REPAINT_DELAY_MS: u32 = 5;
const DEFAULT_TERMINAL_INPUT_DELAY_MS: u32 = 1;
const DEFAULT_TERMINAL_SAFE_KITTY_THUMBNAILS: bool = true;
const DEFAULT_THUMBNAIL_WIDTH: u32 = 800;
const DEFAULT_THUMBNAIL_HEIGHT: u32 = 600;
const DEFAULT_THUMBNAIL_QUALITY: u8 = 85;
const DEFAULT_THUMBNAIL_GRID_COLUMNS: usize = 3;
const DEFAULT_THEME_MODE: &str = "auto";
const DEFAULT_THEME_CHECK_INTERVAL_MS: u64 = 500;
const DEFAULT_TRANSITION_DURATION_SECS: f32 = 1.0;
const DEFAULT_TRANSITION_FPS: u32 = 60;
const DEFAULT_PAIRING_PREVIEW_MATCH_LIMIT: usize = 10;
const DEFAULT_PAIRING_SCREEN_CONTEXT_WEIGHT: f32 = 8.0;
const DEFAULT_PAIRING_VISUAL_WEIGHT: f32 = 5.0;
const DEFAULT_PAIRING_HARMONY_WEIGHT: f32 = 3.0;
const DEFAULT_PAIRING_TAG_WEIGHT: f32 = 2.0;
const DEFAULT_PAIRING_SEMANTIC_WEIGHT: f32 = 7.0;
const DEFAULT_PAIRING_REPETITION_PENALTY_WEIGHT: f32 = 1.0;
const DEFAULT_PAIRING_UNDO_WINDOW_SECS: u64 = 5;
const DEFAULT_PAIRING_AUTO_APPLY_THRESHOLD: f32 = 0.7;
const DEFAULT_PAIRING_MAX_HISTORY_RECORDS: usize = 1000;
const DEFAULT_CLIP_THRESHOLD: f32 = 0.25;
const DEFAULT_CLIP_BATCH_SIZE: usize = 16;
const DEFAULT_WALLPAPER_EXTENSIONS: [&str; 6] = ["jpg", "jpeg", "png", "webp", "bmp", "gif"];
const DEFAULT_KEY_NEXT: &str = "l";
const DEFAULT_KEY_PREV: &str = "h";
const DEFAULT_KEY_APPLY: &str = "Enter";
const DEFAULT_KEY_QUIT: &str = "q";
const DEFAULT_KEY_RANDOM: &str = "r";
const DEFAULT_KEY_TOGGLE_MATCH: &str = "m";
const DEFAULT_KEY_TOGGLE_RESIZE: &str = "f";
const DEFAULT_KEY_NEXT_SCREEN: &str = "Tab";
const DEFAULT_KEY_PREV_SCREEN: &str = "BackTab";

pub(super) fn default_preload_count() -> usize {
    DEFAULT_THUMBNAIL_PRELOAD_COUNT
}

pub(super) fn default_repaint_delay() -> u32 {
    DEFAULT_TERMINAL_REPAINT_DELAY_MS
}

pub(super) fn default_input_delay() -> u32 {
    DEFAULT_TERMINAL_INPUT_DELAY_MS
}

pub(super) fn default_kitty_safe_thumbnails() -> bool {
    DEFAULT_TERMINAL_SAFE_KITTY_THUMBNAILS
}

pub(super) fn default_pairing_preview_match_limit() -> usize {
    DEFAULT_PAIRING_PREVIEW_MATCH_LIMIT
}

pub(super) fn default_pairing_screen_context_weight() -> f32 {
    DEFAULT_PAIRING_SCREEN_CONTEXT_WEIGHT
}

pub(super) fn default_pairing_visual_weight() -> f32 {
    DEFAULT_PAIRING_VISUAL_WEIGHT
}

pub(super) fn default_pairing_harmony_weight() -> f32 {
    DEFAULT_PAIRING_HARMONY_WEIGHT
}

pub(super) fn default_pairing_tag_weight() -> f32 {
    DEFAULT_PAIRING_TAG_WEIGHT
}

pub(super) fn default_pairing_semantic_weight() -> f32 {
    DEFAULT_PAIRING_SEMANTIC_WEIGHT
}

pub(super) fn default_pairing_repetition_penalty_weight() -> f32 {
    DEFAULT_PAIRING_REPETITION_PENALTY_WEIGHT
}

pub(super) fn default_clip_batch_size() -> usize {
    DEFAULT_CLIP_BATCH_SIZE
}

impl Default for WallpaperConfig {
    fn default() -> Self {
        Self {
            directory: crate::utils::picture_dir()
                .map(|path| path.join("wallpapers"))
                .unwrap_or_else(|| PathBuf::from("~/Pictures/wallpapers")),
            extensions: DEFAULT_WALLPAPER_EXTENSIONS
                .into_iter()
                .map(str::to_string)
                .collect(),
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
            aspect_sort: false,
        }
    }
}

impl Default for TransitionConfig {
    fn default() -> Self {
        Self {
            transition_type: TransitionType::Fade,
            duration: DEFAULT_TRANSITION_DURATION_SECS,
            fps: DEFAULT_TRANSITION_FPS,
        }
    }
}

impl Default for ThumbnailConfig {
    fn default() -> Self {
        Self {
            width: DEFAULT_THUMBNAIL_WIDTH,
            height: DEFAULT_THUMBNAIL_HEIGHT,
            quality: DEFAULT_THUMBNAIL_QUALITY,
            grid_columns: DEFAULT_THUMBNAIL_GRID_COLUMNS,
            preload_count: DEFAULT_THUMBNAIL_PRELOAD_COUNT,
        }
    }
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            mode: DEFAULT_THEME_MODE.to_string(),
            check_interval_ms: DEFAULT_THEME_CHECK_INTERVAL_MS,
        }
    }
}

impl Default for TerminalConfig {
    fn default() -> Self {
        Self {
            recommended_repaint_delay: DEFAULT_TERMINAL_REPAINT_DELAY_MS,
            recommended_input_delay: DEFAULT_TERMINAL_INPUT_DELAY_MS,
            hint_shown: false,
            kitty_safe_thumbnails: DEFAULT_TERMINAL_SAFE_KITTY_THUMBNAILS,
        }
    }
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            next: DEFAULT_KEY_NEXT.to_string(),
            prev: DEFAULT_KEY_PREV.to_string(),
            apply: DEFAULT_KEY_APPLY.to_string(),
            quit: DEFAULT_KEY_QUIT.to_string(),
            random: DEFAULT_KEY_RANDOM.to_string(),
            toggle_match: DEFAULT_KEY_TOGGLE_MATCH.to_string(),
            toggle_resize: DEFAULT_KEY_TOGGLE_RESIZE.to_string(),
            next_screen: DEFAULT_KEY_NEXT_SCREEN.to_string(),
            prev_screen: DEFAULT_KEY_PREV_SCREEN.to_string(),
        }
    }
}

impl Default for ClipConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            threshold: DEFAULT_CLIP_THRESHOLD,
            batch_size: DEFAULT_CLIP_BATCH_SIZE,
            show_in_filter: true,
            cache_embeddings: true,
            visual_model_url: None,
            visual_model_sha256: None,
        }
    }
}

impl Default for PairingConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            auto_apply: false,
            undo_window_secs: DEFAULT_PAIRING_UNDO_WINDOW_SECS,
            auto_apply_threshold: DEFAULT_PAIRING_AUTO_APPLY_THRESHOLD,
            max_history_records: DEFAULT_PAIRING_MAX_HISTORY_RECORDS,
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
