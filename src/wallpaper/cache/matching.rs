use crate::screen::Screen;
use crate::wallpaper::{Wallpaper, WallpaperCache};
use rand::Rng;
use walkdir::WalkDir;

impl WallpaperCache {
    pub(super) fn validate(&self) -> bool {
        self.validate_impl(true)
    }

    pub(super) fn validate_for_ai(&self) -> bool {
        self.validate_impl(false)
    }

    fn validate_impl(&self, require_color_data: bool) -> bool {
        if !self.source_dir.exists() {
            return false;
        }

        let sample_size = self.wallpapers.len().min(20);
        let step = if self.wallpapers.len() > sample_size {
            self.wallpapers.len() / sample_size
        } else {
            1
        };

        for (i, wp) in self.wallpapers.iter().enumerate() {
            if i % step != 0 {
                continue;
            }
            if !wp.path.exists() {
                return false;
            }
            if require_color_data && wp.colors.is_empty() {
                return false;
            }
            if wp.modified_at > 0 {
                if let Ok(meta) = std::fs::metadata(&wp.path) {
                    if let Ok(mtime) = meta.modified() {
                        if let Ok(duration) = mtime.duration_since(std::time::UNIX_EPOCH) {
                            if duration.as_secs() > wp.modified_at {
                                return false;
                            }
                        }
                    }
                }
            }
        }

        if self.recursive {
            if let Ok(cache_meta) = std::fs::metadata(Self::cache_path()) {
                if let Ok(cache_mtime) = cache_meta.modified() {
                    let cache_mtime_secs = cache_mtime
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);

                    let dir_changed = WalkDir::new(&self.source_dir)
                        .follow_links(true)
                        .into_iter()
                        .filter_map(|e| e.ok())
                        .filter(|e| e.file_type().is_dir())
                        .filter_map(|e| e.metadata().ok())
                        .filter_map(|m| m.modified().ok())
                        .filter_map(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
                        .any(|d| d.as_secs() > cache_mtime_secs);

                    if dir_changed {
                        return false;
                    }
                }
            }
            return true;
        }

        let current_count = if let Ok(entries) = std::fs::read_dir(&self.source_dir) {
            entries
                .filter_map(|e| e.ok())
                .filter(|e| e.path().is_file() && crate::utils::is_image_file(&e.path()))
                .count()
        } else {
            return false;
        };

        current_count == self.wallpapers.len()
    }

    fn screen_match_key(screen: &Screen) -> String {
        format!(
            "{}:{}x{}:{:?}",
            screen.name, screen.width, screen.height, screen.aspect_category
        )
    }

    fn matching_indices_for_screen(&mut self, screen: &Screen) -> &Vec<usize> {
        let key = Self::screen_match_key(screen);
        if !self.screen_match_indices.contains_key(&key) {
            let indices = self
                .wallpapers
                .iter()
                .enumerate()
                .filter_map(|(idx, wp)| wp.matches_screen(screen).then_some(idx))
                .collect();
            self.screen_match_indices.insert(key.clone(), indices);
        }
        self.screen_match_indices
            .get(&key)
            .expect("screen match cache key inserted")
    }

    pub fn random_for_screen(&mut self, screen: &Screen) -> Option<&Wallpaper> {
        let selected_idx = {
            let matching = self.matching_indices_for_screen(screen);
            if matching.is_empty() {
                None
            } else {
                let idx = rand::thread_rng().gen_range(0..matching.len());
                Some(matching[idx])
            }
        };

        if let Some(cache_idx) = selected_idx {
            return self.wallpapers.get(cache_idx);
        }

        if self.wallpapers.is_empty() {
            return None;
        }

        let idx = rand::thread_rng().gen_range(0..self.wallpapers.len());
        self.wallpapers.get(idx)
    }

    pub fn next_for_screen(&mut self, screen: &Screen) -> Option<&Wallpaper> {
        let current_pos = self.screen_indices.get(&screen.name).copied();
        let (next_pos, cache_idx) = {
            let matching = self.matching_indices_for_screen(screen);
            if matching.is_empty() {
                return None;
            }
            let current = current_pos.unwrap_or_else(|| matching.len().saturating_sub(1));
            let next = (current + 1) % matching.len();
            (next, matching[next])
        };
        self.screen_indices.insert(screen.name.clone(), next_pos);
        self.wallpapers.get(cache_idx)
    }

    pub fn prev_for_screen(&mut self, screen: &Screen) -> Option<&Wallpaper> {
        let current_pos = self.screen_indices.get(&screen.name).copied().unwrap_or(0);
        let (prev_pos, cache_idx) = {
            let matching = self.matching_indices_for_screen(screen);
            if matching.is_empty() {
                return None;
            }
            let prev = if current_pos == 0 {
                matching.len() - 1
            } else {
                current_pos - 1
            };
            (prev, matching[prev])
        };
        self.screen_indices.insert(screen.name.clone(), prev_pos);
        self.wallpapers.get(cache_idx)
    }
}
