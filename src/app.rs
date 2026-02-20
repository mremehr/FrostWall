use crate::pairing::{PairingHistory, PairingStyleMode};
use crate::screen::{self, Screen};
use crate::utils::ColorHarmony;
use crate::wallpaper::{SortMode, WallpaperCache};
use anyhow::Result;
use crossterm::event;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;

mod actions;
mod commands;
mod config;
mod filters;
mod navigation;
mod pairing_ui;
mod runtime;
mod thumbnails;

pub use config::Config;
pub use runtime::run_tui;

/// Request to load a thumbnail in background
pub struct ThumbnailRequest {
    pub cache_idx: usize,
    pub source_path: PathBuf,
    pub generation: u64,
}

/// Response from thumbnail loading
pub struct ThumbnailResponse {
    pub cache_idx: usize,
    pub image: image::DynamicImage,
    pub generation: u64,
}

/// Events from background threads
pub enum AppEvent {
    Key(event::KeyEvent),
    ThumbnailReady(ThumbnailResponse),
    Resize,
    Tick,
}

/// Thumbnail cache size multiplier over visible grid columns.
/// Keeps enough thumbnails for smooth scrolling without overwhelming
/// the terminal graphics protocol. When the cache reaches capacity,
/// all Kitty images are purged from the terminal and the cache is
/// reset — visible thumbnails reload from warm disk cache in one frame.
const THUMBNAIL_CACHE_MULTIPLIER: usize = 8;

/// UI-related transient state (popups, command mode, errors).
pub struct UiState {
    pub should_quit: bool,
    pub show_help: bool,
    pub show_colors: bool,
    pub show_color_picker: bool,
    pub command_mode: bool,
    pub command_buffer: String,
    pub status_message: Option<String>,
    pub pywal_export: bool,
    /// Cached theme (updated on theme-change detection, not every frame)
    pub theme: crate::ui::theme::FrostTheme,
}

impl Default for UiState {
    fn default() -> Self {
        Self {
            should_quit: false,
            show_help: false,
            show_colors: false,
            show_color_picker: false,
            command_mode: false,
            command_buffer: String::new(),
            status_message: None,
            pywal_export: false,
            theme: crate::ui::theme::frost_theme(),
        }
    }
}

/// Filter and sort state.
pub struct FilterState {
    pub sort_mode: SortMode,
    pub active_tag: Option<String>,
    pub active_color: Option<String>,
    pub available_colors: Vec<String>,
    pub color_picker_idx: usize,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            sort_mode: SortMode::Name,
            active_tag: None,
            active_color: None,
            available_colors: Vec::new(),
            color_picker_idx: 0,
        }
    }
}

/// Selection navigation state.
#[derive(Default)]
pub struct SelectionState {
    pub screen_idx: usize,
    pub wallpaper_idx: usize,
    pub filtered_wallpapers: Vec<usize>,
    pub screen_positions: HashMap<usize, usize>,
}

/// Multi-screen wallpaper pairing state.
pub struct PairingState {
    pub history: PairingHistory,
    pub suggestions: Vec<PathBuf>,
    pub current_wallpapers: HashMap<String, PathBuf>,
    pub show_preview: bool,
    pub preview_matches: HashMap<String, Vec<(PathBuf, f32, ColorHarmony)>>,
    pub preview_idx: usize,
    pub style_mode: PairingStyleMode,
}

/// Thumbnail rendering state.
pub struct ThumbnailState {
    pub image_picker: Option<Picker>,
    pub cache: HashMap<usize, Box<dyn StatefulProtocol>>,
    cache_order: Vec<usize>,
    pub loading: std::collections::HashSet<usize>,
    request_tx: Option<SyncSender<ThumbnailRequest>>,
    generation: u64,
}

pub struct App {
    pub screens: Vec<Screen>,
    pub cache: WallpaperCache,
    pub config: Config,
    pub ui: UiState,
    pub selection: SelectionState,
    pub filters: FilterState,
    pub thumbnails: ThumbnailState,
    pub pairing: PairingState,
}

impl App {
    /// Create a new App instance with the given wallpaper directory.
    pub fn new(wallpaper_dir: PathBuf) -> Result<Self> {
        let config = Config::load()?;
        let cache =
            WallpaperCache::load_or_scan_recursive(&wallpaper_dir, config.wallpaper.recursive)?;

        // Try to create image picker for thumbnail rendering
        // from_termios() queries terminal for font size
        // guess_protocol() then detects the best graphics protocol (Kitty, Sixel, etc.)
        let image_picker = Picker::from_termios()
            .ok()
            .map(|mut p| {
                // Actively query terminal for graphics protocol support
                p.guess_protocol();
                p
            })
            .or_else(|| Some(Picker::new((8, 16))));

        // Load pairing history and rebuild affinity scores with corrected formula
        let mut pairing_history = PairingHistory::load(config.pairing.max_history_records)
            .unwrap_or_else(|_| PairingHistory::new(config.pairing.max_history_records));
        pairing_history.rebuild_affinity();

        Ok(Self {
            screens: Vec::new(),
            cache,
            config,
            ui: UiState::default(),
            selection: SelectionState::default(),
            filters: FilterState::default(),
            thumbnails: ThumbnailState {
                image_picker,
                cache: HashMap::new(),
                cache_order: Vec::new(),
                loading: std::collections::HashSet::new(),
                request_tx: None,
                generation: 0,
            },
            pairing: PairingState {
                history: pairing_history,
                suggestions: Vec::new(),
                current_wallpapers: HashMap::new(),
                show_preview: false,
                preview_matches: HashMap::new(),
                preview_idx: 0,
                style_mode: PairingStyleMode::default(),
            },
        })
    }

    /// Detect connected screens and refresh the wallpaper filter.
    pub async fn init_screens(&mut self) -> Result<()> {
        self.screens = screen::detect_screens().await?;
        self.update_filtered_wallpapers();
        Ok(())
    }
}
