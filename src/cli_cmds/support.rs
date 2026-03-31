use anyhow::Result;
use std::path::Path;

use crate::{app, wallpaper};

pub(super) fn load_config() -> Result<app::Config> {
    app::Config::load()
}

pub(super) fn load_cache(
    wallpaper_dir: &Path,
    mode: wallpaper::CacheLoadMode,
) -> Result<(app::Config, wallpaper::WallpaperCache)> {
    let config = load_config()?;
    let cache = load_cache_with_config(wallpaper_dir, &config, mode)?;
    Ok((config, cache))
}

pub(super) fn load_cache_with_config(
    wallpaper_dir: &Path,
    config: &app::Config,
    mode: wallpaper::CacheLoadMode,
) -> Result<wallpaper::WallpaperCache> {
    wallpaper::WallpaperCache::load_or_scan(wallpaper_dir, config.wallpaper.recursive, mode)
}
