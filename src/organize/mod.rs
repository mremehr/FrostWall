mod bucket;
mod dedupe;
mod execute;
mod plan;

pub use bucket::{RenameOptions, RenameScheme};
pub use plan::RenameReport;

use crate::app::Config;
use crate::collections::CollectionStore;
use crate::pairing::PairingHistory;
use crate::thumbnail::ThumbnailCache;
use crate::wallpaper::WallpaperCache;
use anyhow::Result;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Plan and (optionally) execute a wallpaper-rename operation in `dir`,
/// migrating cached state to follow the moved files.
pub fn rename_wallpapers(dir: &Path, options: RenameOptions) -> Result<RenameReport> {
    let mut report = plan::plan_rename_operations(dir, &options)?;
    if options.dry_run || report.planned.is_empty() {
        return Ok(report);
    }

    let stale_thumbnail_names: Vec<String> = report
        .planned
        .iter()
        .map(|rename| ThumbnailCache::cache_file_name_for_source(&rename.from))
        .collect();

    execute::execute_plan(&report.planned)?;

    let mapping: HashMap<PathBuf, PathBuf> = report
        .planned
        .iter()
        .map(|rename| (rename.from.clone(), rename.to.clone()))
        .collect();

    let mut config = Config::load()?;
    report.migration.session_paths_updated = config.remap_session_paths(&mapping)?;
    report.migration.cache_files_updated = WallpaperCache::remap_persisted_paths(&mapping)?;

    let mut collections = CollectionStore::load()?;
    report.migration.collection_paths_updated = collections.remap_paths(&mapping)?;

    let mut history = PairingHistory::load(config.pairing.max_history_records)
        .unwrap_or_else(|_| PairingHistory::new(config.pairing.max_history_records));
    report.migration.pairing_paths_updated = history.remap_paths(&mapping)?;

    report.migration.thumbnails_removed =
        ThumbnailCache::purge_cache_file_names(&stale_thumbnail_names);

    Ok(report)
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| path.display().to_string())
}
