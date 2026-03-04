use crate::wallpaper::{CacheLoadMode, WallpaperCache, CACHE_VERSION};
use anyhow::Result;
use std::fs;
use std::path::Path;

impl WallpaperCache {
    /// Load cache when valid, otherwise scan with the requested mode.
    pub fn load_or_scan(source_dir: &Path, recursive: bool, mode: CacheLoadMode) -> Result<Self> {
        let cache_path = Self::cache_path();

        if cache_path.exists() {
            let data = fs::read_to_string(&cache_path)?;
            if let Ok(mut cache) = serde_json::from_str::<WallpaperCache>(&data) {
                if cache.version != CACHE_VERSION {
                    eprintln!(
                        "Cache format changed (v{} -> v{}), rescanning...",
                        cache.version, CACHE_VERSION
                    );
                    return Self::scan_with_mode(source_dir, recursive, mode);
                }

                if cache.source_dir == source_dir {
                    let previous_recursive = cache.recursive;
                    let recursive_changed = previous_recursive != recursive;
                    cache.recursive = recursive;

                    if !recursive_changed {
                        let valid = match mode {
                            CacheLoadMode::Full => cache.validate(),
                            CacheLoadMode::MetadataOnly => cache.validate_for_ai(),
                        };

                        if valid {
                            cache.rebuild_similarity_profiles();
                            return Ok(cache);
                        }

                        eprintln!("Cache out of date, refreshing incrementally...");
                    } else {
                        eprintln!(
                            "Scan mode changed (recursive: {} -> {}), refreshing incrementally...",
                            previous_recursive, recursive
                        );
                    }

                    match cache.incremental_rescan(recursive) {
                        Ok((added, removed)) => {
                            if added > 0 || removed > 0 {
                                eprintln!("Incremental refresh: +{} / -{} files", added, removed);
                            }

                            if matches!(mode, CacheLoadMode::Full) && cache.ensure_full_color_data()
                            {
                                cache.save()?;
                            }

                            cache.rebuild_similarity_profiles();
                            return Ok(cache);
                        }
                        Err(err) => {
                            eprintln!(
                                "Incremental refresh failed: {}. Falling back to full scan.",
                                err
                            );
                        }
                    }
                }
            }
        }

        Self::scan_with_mode(source_dir, recursive, mode)
    }

    pub(super) fn scan_with_mode(
        source_dir: &Path,
        recursive: bool,
        mode: CacheLoadMode,
    ) -> Result<Self> {
        match mode {
            CacheLoadMode::Full => Self::scan_recursive(source_dir, recursive),
            CacheLoadMode::MetadataOnly => {
                Self::scan_metadata_only_recursive(source_dir, recursive)
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let cache_path = Self::cache_path();

        if let Some(parent) = cache_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let file = fs::File::create(&cache_path)?;
        serde_json::to_writer_pretty(std::io::BufWriter::new(file), self)?;

        Ok(())
    }
}
