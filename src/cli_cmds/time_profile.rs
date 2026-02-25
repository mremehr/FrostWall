use anyhow::Result;
use std::path::Path;

use crate::TimeProfileAction;
use crate::{app, screen, swww, timeprofile, wallpaper};

pub async fn cmd_time_profile(action: TimeProfileAction, wallpaper_dir: &Path) -> Result<()> {
    use timeprofile::TimePeriod;

    let mut config = app::Config::load()?;
    let recursive = config.wallpaper.recursive;

    match action {
        TimeProfileAction::Status => {
            let period = TimePeriod::current();
            let settings = config.time_profiles.settings_for(period);

            println!("{} Current time period: {}", period.emoji(), period.name());
            println!();
            println!(
                "Time profiles: {}",
                if config.time_profiles.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!();
            println!("Settings for {}:", period.name());
            println!(
                "  Brightness range: {:.0}% - {:.0}%",
                settings.brightness_range.0 * 100.0,
                settings.brightness_range.1 * 100.0
            );
            println!("  Preferred tags: {}", settings.preferred_tags.join(", "));
            println!(
                "  Brightness weight: {:.0}%",
                settings.brightness_weight * 100.0
            );
            println!("  Tag weight: {:.0}%", settings.tag_weight * 100.0);
        }
        TimeProfileAction::Enable => {
            config.time_profiles.enabled = true;
            config.save()?;
            println!("Time-based profiles enabled.");
            println!("Run 'frostwall time-profile status' to see current settings.");
        }
        TimeProfileAction::Disable => {
            config.time_profiles.enabled = false;
            config.save()?;
            println!("Time-based profiles disabled.");
        }
        TimeProfileAction::Preview { limit } => {
            let cache =
                wallpaper::WallpaperCache::load_or_scan_recursive(wallpaper_dir, recursive)?;
            let period = TimePeriod::current();

            println!(
                "{} Previewing wallpapers for {} period:",
                period.emoji(),
                period.name()
            );
            println!();

            // Score and sort wallpapers
            let mut scored: Vec<_> = cache
                .wallpapers
                .iter()
                .filter(|wp| !wp.colors.is_empty())
                .map(|wp| {
                    let score = config.time_profiles.score_wallpaper(&wp.colors, &wp.tags);
                    (wp, score)
                })
                .collect();

            scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            for (wp, score) in scored.into_iter().take(limit) {
                let filename = wp.path.file_name().and_then(|n| n.to_str()).unwrap_or("?");
                let tags = if wp.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", wp.tags.join(", "))
                };
                println!("  {:.0}% - {}{}", score * 100.0, filename, tags);
            }
        }
        TimeProfileAction::Apply => {
            let cache =
                wallpaper::WallpaperCache::load_or_scan_recursive(wallpaper_dir, recursive)?;
            let screens = screen::detect_screens().await?;
            let transition = config.transition();
            let period = TimePeriod::current();

            println!(
                "{} Setting wallpapers for {} period...",
                period.emoji(),
                period.name()
            );

            // Get top wallpapers for current time
            let sorted =
                timeprofile::sort_by_time_profile(&cache.wallpapers, &config.time_profiles);

            for (i, screen) in screens.iter().enumerate() {
                if let Some(wp) = sorted.get(i) {
                    swww::set_wallpaper_with_resize(
                        &screen.name,
                        &wp.path,
                        &transition,
                        config.display.resize_mode,
                        &config.display.fill_color,
                    )?;
                    println!(
                        "  {}: {}",
                        screen.name,
                        wp.path.file_name().and_then(|n| n.to_str()).unwrap_or("?")
                    );
                }
            }
        }
    }

    Ok(())
}
