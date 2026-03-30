use anyhow::Result;
use std::path::Path;

use crate::{app, screen, wallpaper, wallpaper_backend};

#[derive(Clone, Copy)]
enum ScreenSelectionMode {
    Random,
    Next,
    Prev,
}

impl ScreenSelectionMode {
    fn select<'a>(
        self,
        cache: &'a mut wallpaper::WallpaperCache,
        screen: &screen::Screen,
    ) -> Option<&'a wallpaper::Wallpaper> {
        match self {
            Self::Random => cache.random_for_screen(screen),
            Self::Next => cache.next_for_screen(screen),
            Self::Prev => cache.prev_for_screen(screen),
        }
    }

    fn persists_selection(self) -> bool {
        !matches!(self, Self::Random)
    }
}

fn load_screen_cache(wallpaper_dir: &Path, recursive: bool) -> Result<wallpaper::WallpaperCache> {
    wallpaper::WallpaperCache::load_or_scan(
        wallpaper_dir,
        recursive,
        wallpaper::CacheLoadMode::MetadataOnly,
    )
}

fn print_empty_wallpaper_hint(wallpaper_dir: &Path) {
    eprintln!("No wallpapers found in: {}", wallpaper_dir.display());
    eprintln!("Run 'frostwall init' to configure your wallpaper directory.");
}

async fn cmd_apply_selection(wallpaper_dir: &Path, mode: ScreenSelectionMode) -> Result<()> {
    let config = app::Config::load()?;
    let screens = screen::detect_screens().await?;
    let mut cache = load_screen_cache(wallpaper_dir, config.wallpaper.recursive)?;

    if cache.wallpapers.is_empty() {
        print_empty_wallpaper_hint(wallpaper_dir);
        return Ok(());
    }

    for screen in &screens {
        if let Some(wp) = mode.select(&mut cache, screen) {
            wallpaper_backend::set_wallpaper_with_resize(
                &config.backend,
                &screen.name,
                &wp.path,
                &config.transition(),
                config.display.resize_mode,
                &config.display.fill_color,
            )?;
            println!("{}: {}", screen.name, wp.path.display());
        }
    }

    if mode.persists_selection() {
        cache.save()?;
    }

    Ok(())
}

pub async fn cmd_random(wallpaper_dir: &Path) -> Result<()> {
    cmd_apply_selection(wallpaper_dir, ScreenSelectionMode::Random).await
}

pub async fn cmd_next(wallpaper_dir: &Path) -> Result<()> {
    cmd_apply_selection(wallpaper_dir, ScreenSelectionMode::Next).await
}

pub async fn cmd_prev(wallpaper_dir: &Path) -> Result<()> {
    cmd_apply_selection(wallpaper_dir, ScreenSelectionMode::Prev).await
}

pub async fn cmd_screens() -> Result<()> {
    let screens = screen::detect_screens().await?;

    for screen in &screens {
        println!(
            "{}: {}x{} ({:?}) - {:?}",
            screen.name, screen.width, screen.height, screen.orientation, screen.aspect_category
        );
    }

    Ok(())
}

pub async fn cmd_scan(wallpaper_dir: &Path) -> Result<()> {
    let recursive = app::Config::load()?.wallpaper.recursive;
    println!("Scanning {}...", wallpaper_dir.display());
    let cache = wallpaper::WallpaperCache::scan_recursive(wallpaper_dir, recursive)?;
    cache.save()?;

    let stats = cache.stats();
    println!("Found {} wallpapers:", stats.total);
    println!("  Ultrawide: {}", stats.ultrawide);
    println!("  Landscape: {}", stats.landscape);
    println!("  Portrait:  {}", stats.portrait);
    println!("  Square:    {}", stats.square);

    Ok(())
}
