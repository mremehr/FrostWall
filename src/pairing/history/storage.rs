use super::*;

impl PairingHistory {
    /// Create new pairing history manager.
    pub fn new(max_records: usize) -> Self {
        let cache_path =
            project_cache_dir(PathBuf::from("/tmp/frostwall")).join("pairing_history.json");

        Self {
            data: PairingHistoryData::default(),
            cache_path,
            current_pairing_start: None,
            undo_state: None,
            max_records,
        }
    }

    /// Load history from cache file.
    pub fn load(max_records: usize) -> Result<Self> {
        let mut history = Self::new(max_records);

        if history.cache_path.exists() {
            let content = std::fs::read_to_string(&history.cache_path)
                .context("Failed to read pairing history")?;
            history.data =
                serde_json::from_str(&content).context("Failed to parse pairing history")?;
        }

        Ok(history)
    }

    /// Save history to cache file.
    pub fn save(&self) -> Result<()> {
        if let Some(parent) = self.cache_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let file = std::fs::File::create(&self.cache_path)?;
        serde_json::to_writer_pretty(std::io::BufWriter::new(file), &self.data)?;
        Ok(())
    }

    /// Rewrite wallpaper paths after a file rename operation.
    pub fn remap_paths(&mut self, mapping: &HashMap<PathBuf, PathBuf>) -> Result<usize> {
        let mut updated = 0;

        for record in &mut self.data.records {
            for path in record.wallpapers.values_mut() {
                if let Some(new_path) = mapping.get(path) {
                    *path = new_path.clone();
                    updated += 1;
                }
            }
        }

        for score in &mut self.data.affinity_scores {
            if let Some(new_path) = mapping.get(&score.wallpaper_a) {
                score.wallpaper_a = new_path.clone();
                updated += 1;
            }
            if let Some(new_path) = mapping.get(&score.wallpaper_b) {
                score.wallpaper_b = new_path.clone();
                updated += 1;
            }
        }

        if updated > 0 {
            self.save()?;
        }

        Ok(updated)
    }

    /// Record a new pairing.
    pub fn record_pairing(&mut self, wallpapers: HashMap<String, PathBuf>, manual: bool) {
        self.end_current_pairing();

        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0);

        self.data.records.push(PairingRecord {
            wallpapers,
            timestamp,
            duration: None,
            manual,
        });
        self.current_pairing_start = Some(timestamp);

        self.prune_old_records();
        let _ = self.save();
    }

    /// Mark end of current pairing (for duration tracking).
    fn end_current_pairing(&mut self) {
        if let Some(start) = self.current_pairing_start.take() {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            let duration = now.saturating_sub(start);

            if let Some(last) = self.data.records.last_mut() {
                last.duration = Some(duration);

                let paths: Vec<_> = last.wallpapers.values().cloned().collect();
                for i in 0..paths.len() {
                    for j in (i + 1)..paths.len() {
                        self.update_affinity(&paths[i], &paths[j], Some(duration));
                    }
                }
            }
        }
    }

    /// Prune old records and stale affinity entries.
    pub(super) fn prune_old_records(&mut self) {
        if self.data.records.len() > self.max_records {
            let to_remove = self.data.records.len() - self.max_records;
            self.data.records.drain(0..to_remove);
        }

        let active_paths: HashSet<&Path> = self
            .data
            .records
            .iter()
            .flat_map(|record| record.wallpapers.values())
            .map(PathBuf::as_path)
            .collect();

        self.data.affinity_scores.retain(|score| {
            active_paths.contains(score.wallpaper_a.as_path())
                && active_paths.contains(score.wallpaper_b.as_path())
        });
    }

    /// Get number of records.
    pub fn record_count(&self) -> usize {
        self.data.records.len()
    }

    /// Get the most recent pairing with multiple screens.
    pub fn get_last_multi_screen_pairing(&self) -> Option<HashMap<String, PathBuf>> {
        self.data
            .records
            .iter()
            .rev()
            .find(|record| record.wallpapers.len() > 1)
            .map(|record| record.wallpapers.clone())
    }
}
