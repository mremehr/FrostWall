use anyhow::Result;
use std::path::Path;

use crate::TagAction;
use crate::{app, utils, wallpaper};

pub fn cmd_tag(action: TagAction, wallpaper_dir: &Path) -> Result<()> {
    let recursive = app::Config::load()?.wallpaper.recursive;
    let mut cache = wallpaper::WallpaperCache::load_or_scan(
        wallpaper_dir,
        recursive,
        wallpaper::CacheLoadMode::MetadataOnly,
    )?;

    match action {
        TagAction::List => {
            let tags = cache.all_tags();
            if tags.is_empty() {
                println!("No tags defined.");
                println!("Add tags with: frostwall tag add <path> <tag>");
            } else {
                println!("Tags:");
                for tag in tags {
                    let count = cache.with_tag(&tag).len();
                    println!("  {} ({})", tag, count);
                }
            }
        }
        TagAction::Add { path, tag } => {
            if cache.add_tag(&path, &tag) {
                cache.save()?;
                println!("✓ Added tag '{}' to {}", tag, path.display());
            } else {
                println!("Wallpaper not found: {}", path.display());
            }
        }
        TagAction::Remove { path, tag } => {
            if cache.remove_tag(&path, &tag) {
                cache.save()?;
                println!("✓ Removed tag '{}' from {}", tag, path.display());
            } else {
                println!("Wallpaper not found: {}", path.display());
            }
        }
        TagAction::Show { tag } => {
            let wallpapers = cache.with_tag(&tag);
            if wallpapers.is_empty() {
                println!("No wallpapers with tag '{}'", tag);
            } else {
                println!("Wallpapers with tag '{}':", tag);
                for wp in wallpapers {
                    println!("  {}", wp.path.display());
                }
            }
        }
    }

    Ok(())
}

pub fn cmd_similar(wallpaper_dir: &Path, target_path: &Path, limit: usize) -> Result<()> {
    let recursive = app::Config::load()?.wallpaper.recursive;
    let cache = wallpaper::WallpaperCache::load_or_scan(
        wallpaper_dir,
        recursive,
        wallpaper::CacheLoadMode::Full,
    )?;

    // Find the target wallpaper
    let target = cache
        .wallpapers
        .iter()
        .find(|wp| wp.path == target_path)
        .or_else(|| {
            // Try matching by filename
            let target_name = target_path.file_name();
            cache
                .wallpapers
                .iter()
                .find(|wp| wp.path.file_name() == target_name)
        });

    let target = match target {
        Some(t) => t,
        None => {
            println!("Wallpaper not found in cache: {}", target_path.display());
            println!("Run 'frostwall scan' first to index wallpapers.");
            return Ok(());
        }
    };

    if target.colors.is_empty() {
        println!("No color data for this wallpaper. Run 'frostwall scan' to extract colors.");
        return Ok(());
    }

    println!("Finding similar wallpapers to: {}", target.path.display());
    println!();

    // Build list of (index, colors) excluding target
    let wallpaper_colors: Vec<(usize, &[String])> = cache
        .wallpapers
        .iter()
        .enumerate()
        .filter(|(_, wp)| wp.path != target.path && !wp.colors.is_empty())
        .map(|(i, wp)| (i, wp.colors.as_slice()))
        .collect();

    let similar = utils::find_similar_wallpapers(&target.colors, &wallpaper_colors, limit);

    if similar.is_empty() {
        println!("No similar wallpapers found.");
    } else {
        println!("Similar wallpapers (by color profile):");
        for (score, idx) in similar {
            let wp = &cache.wallpapers[idx];
            let filename = wp.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
            println!("  {:.0}% - {}", score * 100.0, filename);
        }
    }

    Ok(())
}
