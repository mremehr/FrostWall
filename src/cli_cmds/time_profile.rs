use anyhow::Result;
use std::path::Path;

use super::support::{load_cache_with_config, load_config};
use crate::TimeProfileAction;
use crate::{screen, timeprofile, utils, wallpaper, wallpaper_backend};

pub async fn cmd_time_profile(action: TimeProfileAction, wallpaper_dir: &Path) -> Result<()> {
    use timeprofile::TimePeriod;

    let mut config = load_config()?;

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
                load_cache_with_config(wallpaper_dir, &config, wallpaper::CacheLoadMode::Full)?;
            let period = TimePeriod::current();

            println!(
                "{} Previewing wallpapers for {} period:",
                period.emoji(),
                period.name()
            );
            println!();

            for (wp, score) in
                timeprofile::scored_wallpapers(&cache.wallpapers, &config.time_profiles)
                    .into_iter()
                    .filter(|(wp, _)| !wp.colors.is_empty())
                    .take(limit)
            {
                let tags = if wp.tags.is_empty() {
                    String::new()
                } else {
                    format!(" [{}]", wp.tags.join(", "))
                };
                println!(
                    "  {:.0}% - {}{}",
                    score * 100.0,
                    utils::display_path_name(&wp.path),
                    tags
                );
            }
        }
        TimeProfileAction::Apply => {
            let cache =
                load_cache_with_config(wallpaper_dir, &config, wallpaper::CacheLoadMode::Full)?;
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
                    wallpaper_backend::set_wallpaper_with_resize(
                        &config.backend,
                        &screen.name,
                        &wp.path,
                        &transition,
                        config.display.resize_mode,
                        &config.display.fill_color,
                    )?;
                    println!("  {}: {}", screen.name, utils::display_path_name(&wp.path));
                }
            }
        }
    }

    Ok(())
}
