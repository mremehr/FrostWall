use anyhow::Result;

use crate::CollectionAction;
use crate::{app, collections, pairing, swww};

pub async fn cmd_collection(action: CollectionAction) -> Result<()> {
    match action {
        CollectionAction::List => {
            collections::cmd_collection_list()?;
        }
        CollectionAction::Show { name } => {
            collections::cmd_collection_show(&name)?;
        }
        CollectionAction::Save { name, description } => {
            // Get the most recent pairing from history
            let config = app::Config::load()?;
            let history = pairing::PairingHistory::load(config.pairing.max_history_records)?;

            // Find the most recent record with multiple screens
            let last_pairing = history.get_last_multi_screen_pairing();

            if let Some(wallpapers) = last_pairing {
                if wallpapers.is_empty() {
                    println!("No recent multi-screen pairing found.");
                    println!("Apply wallpapers to multiple screens first, then save.");
                    return Ok(());
                }

                let mut store = collections::CollectionStore::load()?;
                store.add(name.clone(), wallpapers.clone(), description)?;
                println!(
                    "✓ Saved collection '{}' with {} screen(s)",
                    name,
                    wallpapers.len()
                );

                for (screen, path) in &wallpapers {
                    println!("  {}: {}", screen, path.display());
                }
            } else {
                println!("No pairing history found. Apply wallpapers to screens first.");
            }
        }
        CollectionAction::Apply { name } => {
            let store = collections::CollectionStore::load()?;

            if let Some(collection) = store.get(&name) {
                let config = app::Config::load()?;
                let transition = config.transition();

                for (screen_name, wp_path) in &collection.wallpapers {
                    if let Err(e) = swww::set_wallpaper_with_resize(
                        screen_name,
                        wp_path,
                        &transition,
                        config.display.resize_mode,
                        &config.display.fill_color,
                    ) {
                        eprintln!(
                            "Warning: Failed to set {} on {}: {}",
                            wp_path.display(),
                            screen_name,
                            e
                        );
                    } else {
                        println!("✓ {}: {}", screen_name, wp_path.display());
                    }
                }
                println!("Applied collection '{}'", name);
            } else {
                println!("Collection '{}' not found", name);
            }
        }
        CollectionAction::Delete { name } => {
            collections::cmd_collection_delete(&name)?;
        }
    }

    Ok(())
}
