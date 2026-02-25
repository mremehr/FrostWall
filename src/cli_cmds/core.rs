use anyhow::Result;
use std::path::Path;

use crate::{app, screen, swww, wallpaper};

pub async fn cmd_random(wallpaper_dir: &Path) -> Result<()> {
    let recursive = app::Config::load()?.wallpaper.recursive;
    let screens = screen::detect_screens().await?;
    let cache = wallpaper::WallpaperCache::load_or_scan_for_ai_recursive(wallpaper_dir, recursive)?;

    if cache.wallpapers.is_empty() {
        eprintln!("No wallpapers found in: {}", wallpaper_dir.display());
        eprintln!("Run 'frostwall init' to configure your wallpaper directory.");
        return Ok(());
    }

    for screen in &screens {
        if let Some(wp) = cache.random_for_screen(screen) {
            swww::set_wallpaper(&screen.name, &wp.path, &swww::Transition::default())?;
            println!("{}: {}", screen.name, wp.path.display());
        }
    }

    Ok(())
}

pub async fn cmd_next(wallpaper_dir: &Path) -> Result<()> {
    let recursive = app::Config::load()?.wallpaper.recursive;
    let screens = screen::detect_screens().await?;
    let mut cache =
        wallpaper::WallpaperCache::load_or_scan_for_ai_recursive(wallpaper_dir, recursive)?;

    if cache.wallpapers.is_empty() {
        eprintln!("No wallpapers found in: {}", wallpaper_dir.display());
        eprintln!("Run 'frostwall init' to configure your wallpaper directory.");
        return Ok(());
    }

    for screen in &screens {
        if let Some(wp) = cache.next_for_screen(screen) {
            swww::set_wallpaper(&screen.name, &wp.path, &swww::Transition::default())?;
            println!("{}: {}", screen.name, wp.path.display());
        }
    }

    cache.save()?;
    Ok(())
}

pub async fn cmd_prev(wallpaper_dir: &Path) -> Result<()> {
    let recursive = app::Config::load()?.wallpaper.recursive;
    let screens = screen::detect_screens().await?;
    let mut cache =
        wallpaper::WallpaperCache::load_or_scan_for_ai_recursive(wallpaper_dir, recursive)?;

    if cache.wallpapers.is_empty() {
        eprintln!("No wallpapers found in: {}", wallpaper_dir.display());
        eprintln!("Run 'frostwall init' to configure your wallpaper directory.");
        return Ok(());
    }

    for screen in &screens {
        if let Some(wp) = cache.prev_for_screen(screen) {
            swww::set_wallpaper(&screen.name, &wp.path, &swww::Transition::default())?;
            println!("{}: {}", screen.name, wp.path.display());
        }
    }

    cache.save()?;
    Ok(())
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
