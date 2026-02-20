use anyhow::Result;
use std::path::Path;

use crate::PairAction;
use crate::{app, pairing, wallpaper};

pub fn cmd_pair(action: PairAction, wallpaper_dir: &Path) -> Result<()> {
    let config = app::Config::load()?;

    match action {
        PairAction::Stats => {
            let history = pairing::PairingHistory::load(config.pairing.max_history_records)?;
            println!("Pairing Statistics");
            println!("==================");
            println!("  Records: {}", history.record_count());
            println!("  Affinity pairs: {}", history.affinity_count());
            println!();
            println!(
                "Pairing is {}",
                if config.pairing.enabled {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "Auto-apply is {}",
                if config.pairing.auto_apply {
                    "enabled"
                } else {
                    "disabled"
                }
            );
        }
        PairAction::Clear => {
            let history = pairing::PairingHistory::new(config.pairing.max_history_records);
            history.save()?;
            println!("✓ Pairing history cleared");
        }
        PairAction::Suggest { path } => {
            let history = pairing::PairingHistory::load(config.pairing.max_history_records)?;
            let cache = wallpaper::WallpaperCache::load_or_scan(wallpaper_dir)?;

            // Find wallpapers with affinity to the given path
            let mut suggestions: Vec<_> = cache
                .wallpapers
                .iter()
                .filter(|wp| wp.path != path)
                .map(|wp| {
                    let affinity = history.get_affinity(&path, &wp.path);
                    (wp, affinity)
                })
                .filter(|(_, affinity)| *affinity > 0.0)
                .collect();

            suggestions.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

            if suggestions.is_empty() {
                println!("No pairing suggestions for: {}", path.display());
                println!("Use wallpapers together to build pairing history.");
            } else {
                println!("Pairing suggestions for: {}", path.display());
                println!();
                for (wp, affinity) in suggestions.iter().take(10) {
                    println!("  {:.2} - {}", affinity, wp.path.display());
                }
            }
        }
    }

    Ok(())
}
