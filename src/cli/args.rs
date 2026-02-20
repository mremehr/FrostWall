use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "frostwall")]
#[command(author = "MrMattias")]
#[command(version)]
#[command(about = "Intelligent wallpaper manager with screen-aware matching")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Option<Commands>,

    /// Wallpaper directory
    #[arg(short, long)]
    pub(crate) dir: Option<PathBuf>,
}

#[derive(Subcommand)]
pub(crate) enum Commands {
    /// Set a random wallpaper (smart-matched to screens)
    Random,
    /// Set next wallpaper in sequence
    Next,
    /// Set previous wallpaper in sequence
    Prev,
    /// List available screens
    Screens,
    /// Rescan wallpaper directory and update cache
    Scan,
    /// Interactive setup wizard for new users
    Init,
    /// Run watch daemon for automatic wallpaper rotation
    Watch {
        /// Rotation interval (e.g., "30m", "1h", "90s")
        #[arg(short, long, default_value = "30m")]
        interval: String,

        /// Shuffle wallpapers randomly
        #[arg(short, long, default_value = "true")]
        shuffle: bool,

        /// Watch directory for new files
        #[arg(short = 'w', long, default_value = "true")]
        watch_dir: bool,
    },
    /// Manage configuration profiles
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// Manage wallpaper tags
    Tag {
        #[command(subcommand)]
        action: TagAction,
    },
    /// Generate pywal color scheme from wallpaper
    Pywal {
        /// Path to wallpaper image
        path: PathBuf,
        /// Apply colors immediately (xrdb merge)
        #[arg(short, long)]
        apply: bool,
    },
    /// Manage intelligent wallpaper pairing
    Pair {
        #[command(subcommand)]
        action: PairAction,
    },
    /// Auto-tag wallpapers using CLIP AI model (requires --features clip)
    #[cfg(feature = "clip")]
    AutoTag {
        /// Only tag wallpapers missing auto-tags
        #[arg(short, long)]
        incremental: bool,

        /// Confidence threshold (0.0-1.0, default 0.55)
        #[arg(short, long, default_value = "0.55")]
        threshold: f32,

        /// Maximum number of tags per image (0 = unlimited)
        #[arg(short = 'n', long, default_value = "5")]
        max_tags: usize,

        /// Show detailed progress
        #[arg(short, long)]
        verbose: bool,
    },
    /// Manage wallpaper collections (saved presets)
    Collection {
        #[command(subcommand)]
        action: CollectionAction,
    },
    /// Find similar wallpapers based on color profile
    Similar {
        /// Path to wallpaper to find similar ones for
        path: PathBuf,
        /// Maximum number of results
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Manage time-based wallpaper profiles
    TimeProfile {
        #[command(subcommand)]
        action: TimeProfileAction,
    },
    /// Import wallpapers from web galleries (Unsplash, Wallhaven)
    Import {
        #[command(subcommand)]
        action: ImportAction,
    },
}

#[derive(Subcommand)]
pub(crate) enum TagAction {
    /// List all tags
    List,
    /// Add a tag to a wallpaper
    Add {
        /// Path to wallpaper
        path: PathBuf,
        /// Tag to add
        tag: String,
    },
    /// Remove a tag from a wallpaper
    Remove {
        /// Path to wallpaper
        path: PathBuf,
        /// Tag to remove
        tag: String,
    },
    /// Show wallpapers with a specific tag
    Show {
        /// Tag to filter by
        tag: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum PairAction {
    /// Show pairing statistics
    Stats,
    /// Clear all pairing history
    Clear,
    /// Show suggestions for a specific wallpaper
    Suggest {
        /// Path to wallpaper
        path: PathBuf,
    },
}

#[derive(Subcommand)]
pub(crate) enum CollectionAction {
    /// List all saved collections
    List,
    /// Show details of a collection
    Show {
        /// Collection name
        name: String,
    },
    /// Save current wallpapers as a collection
    Save {
        /// Collection name
        name: String,
        /// Optional description
        #[arg(short, long)]
        description: Option<String>,
    },
    /// Apply a saved collection
    Apply {
        /// Collection name
        name: String,
    },
    /// Delete a collection
    Delete {
        /// Collection name
        name: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum ProfileAction {
    /// List all profiles
    List,
    /// Create a new profile
    Create {
        /// Profile name
        name: String,
    },
    /// Delete a profile
    Delete {
        /// Profile name
        name: String,
    },
    /// Switch to a profile
    Use {
        /// Profile name
        name: String,
    },
    /// Set a profile option
    Set {
        /// Profile name
        name: String,
        /// Setting key (directory, match_mode, resize_mode, transition, recursive)
        key: String,
        /// Setting value
        value: String,
    },
}

#[derive(Subcommand)]
pub(crate) enum TimeProfileAction {
    /// Show current time period and settings
    Status,
    /// Enable time-based profiles
    Enable,
    /// Disable time-based profiles
    Disable,
    /// Preview wallpapers matching current time
    Preview {
        /// Maximum number of wallpapers to show
        #[arg(short, long, default_value = "10")]
        limit: usize,
    },
    /// Set a random wallpaper based on current time
    Apply,
}

#[derive(Subcommand)]
pub(crate) enum ImportAction {
    /// Search and import from Unsplash
    Unsplash {
        /// Search query
        query: String,
        /// Number of images to show
        #[arg(short, long, default_value = "10")]
        count: u32,
    },
    /// Search and import from Wallhaven
    Wallhaven {
        /// Search query
        query: String,
        /// Number of images to show
        #[arg(short, long, default_value = "10")]
        count: u32,
    },
    /// Get featured/top wallpapers from Wallhaven
    Featured {
        /// Number of images to show
        #[arg(short, long, default_value = "10")]
        count: u32,
    },
    /// Download a specific image by URL or ID
    Download {
        /// Image URL or Wallhaven ID (e.g., "w8x7y9")
        url: String,
    },
}
