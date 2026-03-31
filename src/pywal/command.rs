use std::borrow::Cow;
use std::path::Path;

use anyhow::{bail, Result};

use super::{apply_colors, export_colors, generate_palette};

/// CLI command to generate and export pywal colors.
pub fn cmd_pywal(wallpaper_path: &Path, apply: bool) -> Result<()> {
    use crate::wallpaper::{CacheLoadMode, WallpaperCache};

    let config = crate::app::Config::load()?;
    let source_dir = config.wallpaper_dir();
    let cache =
        WallpaperCache::load_or_scan(&source_dir, config.wallpaper.recursive, CacheLoadMode::Full)?;

    let colors = wallpaper_colors(&cache, wallpaper_path);
    if colors.is_empty() {
        bail!("No colors extracted from wallpaper");
    }

    let palette = generate_palette(colors.as_ref(), wallpaper_path);
    let cache_path = export_colors(&palette)?;
    print_export_summary(&cache_path);

    if apply {
        apply_colors()?;
        print_apply_summary();
    }

    Ok(())
}

/// Generate pywal colors from currently selected wallpaper in TUI.
pub fn generate_from_wallpaper(colors: &[String], wallpaper_path: &Path) -> Result<()> {
    let palette = generate_palette(colors, wallpaper_path);
    export_colors(&palette)?;
    Ok(())
}

fn wallpaper_colors<'a>(
    cache: &'a crate::wallpaper::WallpaperCache,
    wallpaper_path: &Path,
) -> Cow<'a, [String]> {
    if let Some(wallpaper) = cache
        .wallpapers
        .iter()
        .find(|wallpaper| wallpaper.path == wallpaper_path)
    {
        Cow::Borrowed(wallpaper.colors.as_slice())
    } else {
        Cow::Owned(
            crate::wallpaper::Wallpaper::from_path(wallpaper_path)
                .map(|wallpaper| wallpaper.colors)
                .unwrap_or_default(),
        )
    }
}

fn print_export_summary(cache_path: &Path) {
    println!("✓ Exported colors to {}", cache_path.display());
    println!("  - colors.json");
    println!("  - colors");
    println!("  - colors.sh");
    println!("  - colors.Xresources");
}

fn print_apply_summary() {
    println!("\n✓ Applied colors (xrdb merged)");
    println!("  Restart your terminal or run: source ~/.cache/wal/colors.sh");
}
