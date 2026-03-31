//! pywal integration - Generate terminal color schemes from wallpaper colors.
//!
//! Exports colors to:
//! - ~/.cache/wal/colors.json (JSON format)
//! - ~/.cache/wal/colors (newline-separated hex)
//! - ~/.cache/wal/colors.sh (shell variables)
//! - ~/.cache/wal/colors.Xresources (X11 format)

mod apply;
mod command;
mod export;
mod palette;
mod types;

#[cfg(test)]
mod tests;

pub use apply::apply_colors;
pub use command::{cmd_pywal, generate_from_wallpaper};
pub use export::{export_colors, wal_cache_dir};
pub use palette::generate_palette;
pub use types::{WalColorMap, WalColors, WalSpecial};
