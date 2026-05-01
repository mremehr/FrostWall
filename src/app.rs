use crate::pairing::{PairingHistory, PairingStyleMode};
use crate::screen::{self, Screen};
use crate::utils::ColorHarmony;
use crate::wallpaper::{CacheLoadMode, SortMode, WallpaperCache};
use anyhow::Result;
use crossterm::event;
use lru::LruCache;
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::collections::{HashMap, HashSet};
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::sync::mpsc::SyncSender;
use std::time::{Duration, Instant};

mod actions;
mod analysis;
mod commands;
mod config;
mod filters;
mod navigation;
mod pairing_ui;
mod perf;
mod runtime;
mod thumbnails;

pub use config::Config;
pub use runtime::run_tui;

/// Request to load a thumbnail in background
pub struct ThumbnailRequest {
    pub cache_idx: usize,
    pub source_path: PathBuf,
    pub thumbnail_generation: u64,
    pub analysis_generation: Option<u64>,
}

/// Response from thumbnail loading
pub struct ThumbnailResponse {
    pub cache_idx: usize,
    pub image: image::DynamicImage,
    pub generation: u64,
}

/// Failure while loading a thumbnail in the background.
pub struct ThumbnailFailure {
    pub cache_idx: usize,
    pub generation: u64,
}

/// Request to extract color data in background.
pub struct AnalysisRequest {
    pub cache_idx: usize,
    pub source_path: PathBuf,
    pub generation: u64,
}

/// Response from background color analysis.
pub struct AnalysisResponse {
    pub cache_idx: usize,
    pub colors: Vec<String>,
    pub color_weights: Vec<f32>,
    pub generation: u64,
}

/// Failure while extracting color data in the background.
pub struct AnalysisFailure {
    pub cache_idx: usize,
    pub generation: u64,
}

/// Events from background threads
pub enum AppEvent {
    Key(event::KeyEvent),
    ThumbnailReady(ThumbnailResponse),
    ThumbnailFailed(ThumbnailFailure),
    AnalysisReady(AnalysisResponse),
    AnalysisFailed(AnalysisFailure),
    Resize,
    Tick,
}

/// Thumbnail cache size multiplier over visible grid columns.
/// Keeps enough thumbnails for smooth scrolling without overwhelming
/// the terminal graphics protocol. When the cache reaches capacity,
/// all Kitty images are purged from the terminal and the cache is
/// reset — visible thumbnails reload from warm disk cache in one frame.
const THUMBNAIL_CACHE_MULTIPLIER: usize = 8;
pub(super) const THUMBNAIL_CACHE_HARD_CAP: NonZeroUsize = NonZeroUsize::new(200).unwrap();
const PAIRING_SUGGESTION_DEBOUNCE: Duration = Duration::from_millis(120);

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
    pub aspect_sort_enabled: bool,
    pub active_tag: Option<String>,
    pub active_color: Option<String>,
    pub available_colors: Vec<String>,
    pub color_picker_idx: usize,
}

impl Default for FilterState {
    fn default() -> Self {
        Self {
            sort_mode: SortMode::Name,
            aspect_sort_enabled: false,
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
    pub suggestions_dirty: bool,
    pub suggestions_due_at: Option<Instant>,
    pub current_wallpapers: HashMap<String, PathBuf>,
    pub show_preview: bool,
    pub preview_matches: HashMap<String, Vec<(PathBuf, f32, ColorHarmony)>>,
    pub preview_idx: usize,
    pub style_mode: PairingStyleMode,
}

/// Thumbnail rendering state.
pub struct ThumbnailState {
    pub image_picker: Option<Picker>,
    pub cache: LruCache<usize, Box<dyn StatefulProtocol>>,
    pub loading: HashSet<usize>,
    request_tx: Option<SyncSender<ThumbnailRequest>>,
    generation: u64,
}

/// Background color-analysis state used for fast-start TUI sessions.
pub struct AnalysisState {
    loading: HashSet<usize>,
    request_tx: Option<SyncSender<AnalysisRequest>>,
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
    pub analysis: AnalysisState,
    pub pairing: PairingState,
}

impl App {
    /// Create a new App instance with the given wallpaper directory.
    pub fn new(wallpaper_dir: PathBuf) -> Result<Self> {
        let config = Config::load()?;
        let cache = WallpaperCache::load_or_scan(
            &wallpaper_dir,
            config.wallpaper.recursive,
            CacheLoadMode::MetadataOnly,
        )?;

        // Try to create image picker for thumbnail rendering
        // from_termios() queries terminal for font size
        // and config can force a safer protocol on Kitty.
        let image_picker = Some(Self::new_thumbnail_picker(&config));

        // Load pairing history and rebuild affinity scores with corrected formula
        let mut pairing_history = PairingHistory::load(config.pairing.max_history_records)
            .unwrap_or_else(|_| PairingHistory::new(config.pairing.max_history_records));
        // Only rebuild if affinity scores are missing (first run / empty cache).
        // On a normal start, the scores are already persisted to disk and valid.
        if pairing_history.affinity_count() == 0 && pairing_history.record_count() > 0 {
            pairing_history.rebuild_affinity();
        }
        let filters = FilterState {
            aspect_sort_enabled: config.display.aspect_sort,
            ..FilterState::default()
        };

        Ok(Self {
            screens: Vec::new(),
            cache,
            config,
            ui: UiState::default(),
            selection: SelectionState::default(),
            filters,
            thumbnails: ThumbnailState {
                image_picker,
                cache: LruCache::new(THUMBNAIL_CACHE_HARD_CAP),
                loading: HashSet::new(),
                request_tx: None,
                generation: 0,
            },
            analysis: AnalysisState {
                loading: HashSet::new(),
                request_tx: None,
                generation: 0,
            },
            pairing: PairingState {
                history: pairing_history,
                suggestions: Vec::new(),
                suggestions_dirty: false,
                suggestions_due_at: None,
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
        self.selection.screen_idx = 0;
        // restore_last_selection handles all per-screen filtering and the final
        // full update (including thumbnail reset + scheduling pairing suggestions).
        self.restore_last_selection();
        Ok(())
    }
}
