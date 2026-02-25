use crate::app::Config;
use crate::screen;
use crate::swww::ResizeMode;
use crate::wallpaper::MatchMode;
use anyhow::Result;
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use std::path::PathBuf;

/// Interactive setup wizard for new users
pub async fn run_init() -> Result<()> {
    let theme = ColorfulTheme::default();

    println!("\n❄️  FrostWall Setup Wizard ❄️\n");
    println!("Let's configure your wallpaper manager.\n");

    // Check if config already exists
    let config_path = Config::config_path();
    if config_path.exists() {
        let overwrite = Confirm::with_theme(&theme)
            .with_prompt("Config file already exists. Overwrite?")
            .default(false)
            .interact()?;

        if !overwrite {
            println!("Setup cancelled.");
            return Ok(());
        }
    }

    // Step 1: Wallpaper directory
    let default_dir = dirs::picture_dir()
        .map(|p| p.join("wallpapers"))
        .unwrap_or_else(|| PathBuf::from("~/Pictures/wallpapers"));

    let wallpaper_dir: String = Input::with_theme(&theme)
        .with_prompt("Wallpaper directory")
        .default(default_dir.to_string_lossy().to_string())
        .interact_text()?;

    // Expand ~ and validate
    let expanded_dir = expand_tilde(&wallpaper_dir);
    if !expanded_dir.exists() {
        let create = Confirm::with_theme(&theme)
            .with_prompt(format!(
                "Directory doesn't exist. Create {}?",
                expanded_dir.display()
            ))
            .default(true)
            .interact()?;

        if create {
            std::fs::create_dir_all(&expanded_dir)?;
            println!("✓ Created {}", expanded_dir.display());
        }
    }

    // Step 2: Detect screens
    println!("\nDetecting screens...");
    match screen::detect_screens().await {
        Ok(screens) => {
            println!("Found {} screen(s):", screens.len());
            for s in &screens {
                println!(
                    "  • {} ({}x{}, {:?})",
                    s.name, s.width, s.height, s.aspect_category
                );
            }
        }
        Err(e) => {
            println!("⚠ Could not detect screens: {}", e);
            println!("  Make sure niri or wlr-randr is available.");
        }
    }

    // Step 3: Match mode
    let match_modes = vec![
        "Flexible - Compatible aspect ratios (recommended)",
        "Strict - Only exact aspect matches",
        "All - Show all wallpapers",
    ];

    let match_idx = Select::with_theme(&theme)
        .with_prompt("\nAspect ratio matching")
        .items(&match_modes)
        .default(0)
        .interact()?;

    let match_mode = match match_idx {
        0 => MatchMode::Flexible,
        1 => MatchMode::Strict,
        _ => MatchMode::All,
    };

    // Step 4: Resize mode
    let resize_modes = vec![
        "Fit - Preserve aspect ratio (letterbox if needed)",
        "Crop - Fill screen, crop excess",
        "Center - No resize, center image",
        "Stretch - Fill screen (distorts)",
    ];

    let resize_idx = Select::with_theme(&theme)
        .with_prompt("How to fit wallpapers")
        .items(&resize_modes)
        .default(0)
        .interact()?;

    let resize_mode = match resize_idx {
        0 => ResizeMode::Fit,
        1 => ResizeMode::Crop,
        2 => ResizeMode::No,
        _ => ResizeMode::Stretch,
    };

    // Step 5: Transition
    let transitions = vec![
        "fade - Smooth crossfade (recommended)",
        "wipe - Wipe from edge",
        "grow - Grow from center",
        "center - Circular reveal",
        "none - Instant change",
    ];

    let trans_idx = Select::with_theme(&theme)
        .with_prompt("Transition effect")
        .items(&transitions)
        .default(0)
        .interact()?;

    let transition_type = match trans_idx {
        0 => "fade",
        1 => "wipe",
        2 => "grow",
        3 => "center",
        _ => "none",
    };

    // Step 6: Scan subdirectories
    let recursive = Confirm::with_theme(&theme)
        .with_prompt("Scan subdirectories for wallpapers?")
        .default(false)
        .interact()?;

    // Build config
    let mut config = Config::default();
    config.wallpaper.directory = PathBuf::from(wallpaper_dir);
    config.wallpaper.recursive = recursive;
    config.display.match_mode = match_mode;
    config.display.resize_mode = resize_mode;
    config.transition.transition_type = transition_type.to_string();

    // Save config
    config.save()?;
    println!("\n✓ Config saved to {}", config_path.display());

    // Offer to scan wallpapers
    let scan_now = Confirm::with_theme(&theme)
        .with_prompt("Scan wallpaper directory now?")
        .default(true)
        .interact()?;

    if scan_now {
        println!("\nScanning {}...", expanded_dir.display());
        match crate::wallpaper::WallpaperCache::scan_recursive(&expanded_dir, recursive) {
            Ok(cache) => {
                cache.save()?;
                let stats = cache.stats();
                println!("✓ Found {} wallpapers:", stats.total);
                println!("  • Ultrawide: {}", stats.ultrawide);
                println!("  • Landscape: {}", stats.landscape);
                println!("  • Portrait:  {}", stats.portrait);
                println!("  • Square:    {}", stats.square);
            }
            Err(e) => {
                println!("⚠ Scan failed: {}", e);
            }
        }
    }

    println!("\n❄️  Setup complete! Run 'frostwall' to launch the TUI.\n");

    Ok(())
}

fn expand_tilde(path: &str) -> PathBuf {
    crate::utils::expand_tilde(path)
}
