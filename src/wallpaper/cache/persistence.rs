use crate::wallpaper::{CacheLoadMode, WallpaperCache, CACHE_VERSION};
use anyhow::Result;
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

impl WallpaperCache {
    /// Load cache when valid, otherwise scan with the requested mode.
    pub fn load_or_scan(source_dir: &Path, recursive: bool, mode: CacheLoadMode) -> Result<Self> {
        let cache_path = Self::cache_path();

        if cache_path.exists() {
            if let Ok(mut cache) = Self::read_cache_file(&cache_path) {
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
        Self::write_cache_file(&cache_path, self)
    }

    fn write_cache_file(path: &Path, cache: &Self) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let temp_path = path.with_extension("json.tmp");
        let file = fs::File::create(&temp_path)?;
        let mut writer = BufWriter::new(file);
        serde_json::to_writer(&mut writer, cache)?;
        writer.flush()?;
        drop(writer);
        fs::rename(temp_path, path)?;
        Ok(())
    }

    fn read_cache_file(path: &Path) -> Result<Self> {
        let file = fs::File::open(path)?;
        let reader = BufReader::new(file);
        Ok(serde_json::from_reader(reader)?)
    }

    /// Serialize cache to a caller-specified path (used in tests).
    #[cfg(test)]
    pub(crate) fn save_to(&self, path: &Path) -> Result<()> {
        Self::write_cache_file(path, self)
    }

    /// Deserialize cache from a caller-specified path (used in tests).
    #[cfg(test)]
    pub(crate) fn load_from(path: &Path) -> Result<Self> {
        Self::read_cache_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::screen::AspectCategory;
    use crate::wallpaper::{Wallpaper, WallpaperCache, CACHE_VERSION};
    use std::collections::HashMap;

    fn minimal_cache(dir: &Path) -> WallpaperCache {
        WallpaperCache {
            version: CACHE_VERSION,
            wallpapers: vec![Wallpaper {
                path: dir.join("a.jpg"),
                width: 1920,
                height: 1080,
                aspect_category: AspectCategory::Landscape,
                colors: vec!["#112233".into()],
                color_weights: vec![1.0],
                tags: vec!["nature".into()],
                auto_tags: vec![],
                embedding: None,
                file_size: 0,
                modified_at: 0,
            }],
            source_dir: dir.to_path_buf(),
            screen_indices: HashMap::new(),
            recursive: false,
            screen_match_indices: HashMap::new(),
            similarity_profiles: Vec::new(),
        }
    }

    #[test]
    fn test_save_creates_file() {
        let dir = tempfile::tempdir().unwrap();
        let cache = minimal_cache(dir.path());
        let path = dir.path().join("cache.json");
        cache.save_to(&path).unwrap();
        assert!(path.exists(), "save_to should create the cache file");
    }

    #[test]
    fn test_save_load_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let original = minimal_cache(dir.path());
        let path = dir.path().join("cache.json");
        original.save_to(&path).unwrap();
        let loaded = WallpaperCache::load_from(&path).unwrap();
        assert_eq!(loaded.wallpapers.len(), original.wallpapers.len());
        assert_eq!(loaded.wallpapers[0].path, original.wallpapers[0].path);
        assert_eq!(loaded.wallpapers[0].colors, original.wallpapers[0].colors);
        assert_eq!(loaded.source_dir, original.source_dir);
        assert_eq!(loaded.recursive, original.recursive);
    }

    #[test]
    fn test_save_writes_compact_json() {
        let dir = tempfile::tempdir().unwrap();
        let cache = minimal_cache(dir.path());
        let path = dir.path().join("cache.json");
        cache.save_to(&path).unwrap();
        let contents = fs::read_to_string(&path).unwrap();
        assert!(!contents.contains('\n'));
    }

    #[test]
    fn test_load_missing_file_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("nonexistent.json");
        assert!(
            WallpaperCache::load_from(&missing).is_err(),
            "loading a missing file should return an error"
        );
    }

    #[test]
    fn test_cache_version_mismatch_detected() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = minimal_cache(dir.path());
        // Write a cache with a stale version number.
        cache.version = 0;
        let path = dir.path().join("cache.json");
        cache.save_to(&path).unwrap();
        let loaded = WallpaperCache::load_from(&path).unwrap();
        assert_ne!(
            loaded.version, CACHE_VERSION,
            "loaded version should differ from current CACHE_VERSION"
        );
    }
}
