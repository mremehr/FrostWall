use anyhow::Result;
use std::path::Path;

use crate::organize::{rename_wallpapers, RenameOptions};
use crate::OrganizeAction;

pub fn cmd_organize(action: OrganizeAction, wallpaper_dir: &Path) -> Result<()> {
    match action {
        OrganizeAction::Rename {
            dry_run,
            compact,
            warn_content_dupes,
            scheme,
        } => {
            let report = rename_wallpapers(
                wallpaper_dir,
                RenameOptions {
                    dry_run,
                    compact,
                    warn_content_dupes,
                    scheme,
                },
            )?;

            if report.total_files == 0 {
                println!(
                    "No supported wallpapers found in {}",
                    wallpaper_dir.display()
                );
                return Ok(());
            }

            if dry_run {
                println!("Dry run: {} planned rename(s)", report.planned.len());
            } else {
                println!("Renamed {} wallpaper(s)", report.planned.len());
            }

            if report.skipped_already_named > 0 {
                println!(
                    "Skipped {} already-numbered file(s)",
                    report.skipped_already_named
                );
            }

            for rename in &report.planned {
                println!("  {} -> {}", rename.from.display(), rename.to.display());
            }

            if !dry_run && !report.planned.is_empty() {
                println!(
                    "Updated cache files: {}, session refs: {}, collection refs: {}, pairing refs: {}, thumbnails purged: {}",
                    report.migration.cache_files_updated,
                    report.migration.session_paths_updated,
                    report.migration.collection_paths_updated,
                    report.migration.pairing_paths_updated,
                    report.migration.thumbnails_removed
                );
            }

            for warning in &report.warnings.duplicate_numbered_stems {
                println!("Warning: duplicate numbered stem {}", warning);
            }
            for warning in &report.warnings.content_duplicates {
                println!("Warning: content duplicate {}", warning);
            }
            for warning in &report.warnings.unreadable_files {
                println!("Warning: unreadable file {}", warning);
            }
        }
    }

    Ok(())
}
