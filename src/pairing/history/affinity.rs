use super::*;

impl PairingHistory {
    /// Update affinity score between two wallpapers.
    pub(super) fn update_affinity(&mut self, wp_a: &Path, wp_b: &Path, duration: Option<u64>) {
        let (a, b) = Self::ordered_pair(wp_a, wp_b);

        let entry = self
            .data
            .affinity_scores
            .iter_mut()
            .find(|score| score.wallpaper_a == a && score.wallpaper_b == b);

        if let Some(score) = entry {
            score.pair_count += 1;
            if let Some(duration_secs) = duration {
                let total_duration = score.avg_duration_secs * (score.pair_count - 1) as f32;
                score.avg_duration_secs =
                    (total_duration + duration_secs as f32) / score.pair_count as f32;
            }
            score.score = Self::calculate_base_score(score.pair_count, score.avg_duration_secs);
        } else {
            self.data.affinity_scores.push(AffinityScore {
                wallpaper_a: a.to_path_buf(),
                wallpaper_b: b.to_path_buf(),
                score: Self::calculate_base_score(1, duration.unwrap_or(0) as f32),
                pair_count: 1,
                avg_duration_secs: duration.unwrap_or(0) as f32,
            });
        }
    }

    /// Calculate base affinity score from pairing stats.
    /// Normalized to roughly 0.0–1.0 so it doesn't dominate other features.
    pub(super) fn calculate_base_score(pair_count: u32, avg_duration_secs: f32) -> f32 {
        let count_score = (pair_count as f32).ln_1p() / AFFINITY_PAIR_COUNT_SATURATION.ln_1p();
        let duration_score = (avg_duration_secs / AFFINITY_DURATION_TARGET_SECS).min(1.0);

        (count_score * AFFINITY_COUNT_WEIGHT + duration_score * AFFINITY_DURATION_WEIGHT).min(1.0)
    }

    /// Get ordered pair of paths (for consistent key).
    pub(super) fn ordered_pair<'a>(a: &'a Path, b: &'a Path) -> (&'a Path, &'a Path) {
        if a < b {
            (a, b)
        } else {
            (b, a)
        }
    }

    /// Get affinity score between two wallpapers.
    pub fn get_affinity(&self, wp_a: &Path, wp_b: &Path) -> f32 {
        let (a, b) = Self::ordered_pair(wp_a, wp_b);

        self.data
            .affinity_scores
            .iter()
            .find(|score| score.wallpaper_a == a && score.wallpaper_b == b)
            .map(|score| score.score)
            .unwrap_or(0.0)
    }

    /// Rebuild affinity scores from scratch based on current records.
    /// Use this after fixing bugs in the scoring logic to reset stale data.
    pub fn rebuild_affinity(&mut self) {
        self.data.affinity_scores.clear();

        let pairs: Vec<(Vec<PathBuf>, Option<u64>)> = self
            .data
            .records
            .iter()
            .map(|record| {
                let paths: Vec<PathBuf> = record.wallpapers.values().cloned().collect();
                (paths, record.duration)
            })
            .collect();

        for (paths, duration) in &pairs {
            for i in 0..paths.len() {
                for j in (i + 1)..paths.len() {
                    self.update_affinity(&paths[i], &paths[j], *duration);
                }
            }
        }

        let _ = self.save();
    }

    /// Get number of affinity pairs.
    pub fn affinity_count(&self) -> usize {
        self.data.affinity_scores.len()
    }
}
