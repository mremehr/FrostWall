use anyhow::Result;
use std::path::Path;

use super::support::load_cache;
use crate::TagAction;
use crate::{utils, wallpaper};

pub fn cmd_tag(action: TagAction, wallpaper_dir: &Path) -> Result<()> {
    let (_, mut cache) = load_cache(wallpaper_dir, wallpaper::CacheLoadMode::MetadataOnly)?;

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
    let (_, mut cache) = load_cache(wallpaper_dir, wallpaper::CacheLoadMode::Full)?;
    cache.ensure_similarity_profiles();

    // Find the target wallpaper
    let target_idx = cache
        .wallpapers
        .iter()
        .enumerate()
        .find(|(_, wp)| wp.path == target_path)
        .or_else(|| {
            // Try matching by filename
            let target_name = target_path.file_name();
            cache
                .wallpapers
                .iter()
                .enumerate()
                .find(|(_, wp)| wp.path.file_name() == target_name)
        });

    let target_idx = match target_idx {
        Some((idx, _)) => idx,
        None => {
            println!("Wallpaper not found in cache: {}", target_path.display());
            println!("Run 'frostwall scan' first to index wallpapers.");
            return Ok(());
        }
    };
    let target = &cache.wallpapers[target_idx];
    let target_profile = &cache.similarity_profiles[target_idx];

    if target.colors.is_empty() {
        println!("No color data for this wallpaper. Run 'frostwall scan' to extract colors.");
        return Ok(());
    }

    println!("Finding similar wallpapers to: {}", target.path.display());
    println!();

    let similar = utils::find_similar_wallpapers_with_profiles_iter(
        &target.colors,
        target_profile,
        cache.wallpapers.iter().enumerate().filter_map(|(i, wp)| {
            if i == target_idx || wp.colors.is_empty() {
                return None;
            }

            cache
                .similarity_profiles
                .get(i)
                .map(|profile| (i, wp.colors.as_slice(), profile))
        }),
        limit,
    );

    if similar.is_empty() {
        println!("No similar wallpapers found.");
    } else {
        println!("Similar wallpapers (by color profile):");
        for (score, idx) in similar {
            let wp = &cache.wallpapers[idx];
            println!(
                "  {:.0}% - {}",
                score * 100.0,
                utils::display_path_name(&wp.path)
            );
        }
    }

    Ok(())
}
