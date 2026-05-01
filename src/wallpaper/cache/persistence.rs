use crate::wallpaper::{CacheLoadMode, CachePayload, WallpaperCache, CACHE_VERSION};
use anyhow::Result;
use std::collections::HashMap;
use std::fs;
use std::io::{BufReader, BufWriter, Write};
use std::path::Path;

impl WallpaperCache {
    /// Load cache when valid, otherwise scan with the requested mode.
    pub fn load_or_scan(source_dir: &Path, recursive: bool, mode: CacheLoadMode) -> Result<Self> {
        match mode {
            CacheLoadMode::Startup => Self::load_startup_or_scan(source_dir, recursive),
            CacheLoadMode::Full | CacheLoadMode::MetadataOnly => {
                Self::load_standard_or_scan(source_dir, recursive, mode)
            }
        }
    }

    pub(super) fn scan_with_mode(
        source_dir: &Path,
        recursive: bool,
        mode: CacheLoadMode,
    ) -> Result<Self> {
        match mode {
            CacheLoadMode::Full => Self::scan_recursive(source_dir, recursive),
            CacheLoadMode::MetadataOnly | CacheLoadMode::Startup => {
                Self::scan_metadata_only_recursive(source_dir, recursive)
            }
        }
    }

    pub fn save(&self) -> Result<()> {
        let cache_path = Self::cache_path();
        let full_cache = if self.payload == CachePayload::Startup {
            self.merge_for_full_save()?
        } else {
            let mut cache = self.clone();
            cache.payload = CachePayload::Full;
            cache
        };

        Self::write_cache_file(&cache_path, &full_cache)?;
        let startup_cache = full_cache.startup_snapshot();
        Self::write_cache_file(&Self::startup_cache_path(), &startup_cache)
    }

    fn load_standard_or_scan(
        source_dir: &Path,
        recursive: bool,
        mode: CacheLoadMode,
    ) -> Result<Self> {
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
                            CacheLoadMode::MetadataOnly | CacheLoadMode::Startup => {
                                cache.validate_for_ai()
                            }
                        };

                        if valid {
                            cache.payload = CachePayload::Full;
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

                            cache.payload = CachePayload::Full;
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

    fn load_startup_or_scan(source_dir: &Path, recursive: bool) -> Result<Self> {
        let startup_path = Self::startup_cache_path();
        if startup_path.exists() {
            if let Ok(mut cache) = Self::read_cache_file(&startup_path) {
                cache.payload = CachePayload::Startup;
                if cache.version == CACHE_VERSION
                    && cache.source_dir == source_dir
                    && cache.recursive == recursive
                    && cache.validate_for_ai()
                {
                    cache.rebuild_similarity_profiles();
                    return Ok(cache);
                }
            }
        }

        let cache =
            Self::load_standard_or_scan(source_dir, recursive, CacheLoadMode::MetadataOnly)?;
        let startup_cache = cache.startup_snapshot();
        let _ = Self::write_cache_file(&startup_path, &startup_cache);
        Ok(startup_cache)
    }

    fn startup_snapshot(&self) -> Self {
        let mut cache = self.clone();
        for wallpaper in &mut cache.wallpapers {
            wallpaper.colors.clear();
            wallpaper.color_weights.clear();
            wallpaper.embedding = None;
        }
        cache.similarity_profiles.clear();
        cache.screen_match_indices.clear();
        cache.payload = CachePayload::Startup;
        cache
    }

    fn merge_for_full_save(&self) -> Result<Self> {
        let full_path = Self::cache_path();
        if !full_path.exists() {
            return Ok(self.merge_for_full_save_from(None));
        }

        let existing = Self::read_cache_file(&full_path).ok();
        Ok(self.merge_for_full_save_from(existing))
    }

    fn merge_for_full_save_from(&self, existing: Option<Self>) -> Self {
        let mut merged = self.clone();
        merged.payload = CachePayload::Full;

        let Some(existing) = existing else {
            return merged;
        };
        if existing.version != CACHE_VERSION || existing.source_dir != self.source_dir {
            return merged;
        }

        let mut existing_by_path: HashMap<_, _> = existing
            .wallpapers
            .into_iter()
            .map(|wallpaper| (wallpaper.path.clone(), wallpaper))
            .collect();

        for wallpaper in &mut merged.wallpapers {
            let Some(existing_wallpaper) = existing_by_path.remove(&wallpaper.path) else {
                continue;
            };

            if wallpaper.tags.is_empty() && !existing_wallpaper.tags.is_empty() {
                wallpaper.tags = existing_wallpaper.tags.clone();
            }
            if wallpaper.auto_tags.is_empty() && !existing_wallpaper.auto_tags.is_empty() {
                wallpaper.auto_tags = existing_wallpaper.auto_tags.clone();
            }
            if wallpaper.colors.is_empty() && !existing_wallpaper.colors.is_empty() {
                wallpaper.colors = existing_wallpaper.colors.clone();
                wallpaper.color_weights = existing_wallpaper.color_weights.clone();
            }
            if wallpaper.embedding.is_none() {
                wallpaper.embedding = existing_wallpaper.embedding.clone();
            }
        }

        merged
    }

    fn remap_paths_in_place(
        &mut self,
        mapping: &HashMap<std::path::PathBuf, std::path::PathBuf>,
    ) -> bool {
        let mut changed = false;

        for wallpaper in &mut self.wallpapers {
            if let Some(updated) = mapping.get(&wallpaper.path) {
                wallpaper.path = updated.clone();
                changed = true;
            }
        }

        if changed {
            self.wallpapers.sort_by(|a, b| a.path.cmp(&b.path));
            self.screen_match_indices.clear();
            self.rebuild_similarity_profiles();
        }

        changed
    }

    fn remap_cache_file(
        path: &Path,
        payload: CachePayload,
        mapping: &HashMap<std::path::PathBuf, std::path::PathBuf>,
    ) -> Result<bool> {
        if mapping.is_empty() || !path.exists() {
            return Ok(false);
        }

        let mut cache = Self::read_cache_file(path)?;
        if !cache.remap_paths_in_place(mapping) {
            return Ok(false);
        }

        cache.payload = payload;
        Self::write_cache_file(path, &cache)?;
        Ok(true)
    }

    pub fn remap_persisted_paths(
        mapping: &HashMap<std::path::PathBuf, std::path::PathBuf>,
    ) -> Result<usize> {
        let mut updated_files = 0;

        if Self::remap_cache_file(&Self::cache_path(), CachePayload::Full, mapping)? {
            updated_files += 1;
        }
        if Self::remap_cache_file(&Self::startup_cache_path(), CachePayload::Startup, mapping)? {
            updated_files += 1;
        }

        Ok(updated_files)
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
    use crate::wallpaper::{CachePayload, Wallpaper, WallpaperCache, CACHE_VERSION};
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
            payload: CachePayload::Full,
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
    fn test_startup_snapshot_drops_heavy_fields() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = minimal_cache(dir.path());
        cache.wallpapers[0].embedding = Some(vec![0.1, 0.2, 0.3]);

        let startup = cache.startup_snapshot();

        assert!(startup.wallpapers[0].colors.is_empty());
        assert!(startup.wallpapers[0].color_weights.is_empty());
        assert!(startup.wallpapers[0].embedding.is_none());
        assert_eq!(startup.payload, CachePayload::Startup);
    }

    #[test]
    fn test_merge_for_full_save_restores_existing_embedding() {
        let dir = tempfile::tempdir().unwrap();
        let mut existing = minimal_cache(dir.path());
        existing.wallpapers[0].embedding = Some(vec![0.1, 0.2, 0.3]);

        let startup = existing.startup_snapshot();
        let merged = startup.merge_for_full_save_from(Some(existing));

        assert_eq!(merged.wallpapers[0].embedding, Some(vec![0.1, 0.2, 0.3]));
    }

    #[test]
    fn test_remap_paths_in_place_updates_wallpaper_paths() {
        let dir = tempfile::tempdir().unwrap();
        let mut cache = minimal_cache(dir.path());
        let old_path = cache.wallpapers[0].path.clone();
        let new_path = dir.path().join("renamed.png");
        let mapping = HashMap::from([(old_path, new_path.clone())]);

        assert!(cache.remap_paths_in_place(&mapping));
        assert_eq!(cache.wallpapers[0].path, new_path);
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
