use super::{App, Config, ThumbnailRequest, ThumbnailResponse, THUMBNAIL_CACHE_MULTIPLIER};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::sync::mpsc::SyncSender;

const THUMBNAIL_MAX_IN_FLIGHT_MULTIPLIER: usize = 2;

impl App {
    /// Request a thumbnail to be loaded in background.
    pub fn request_thumbnail(&mut self, cache_idx: usize) {
        // Bounds check.
        if cache_idx >= self.cache.wallpapers.len() {
            return;
        }

        // Skip if already loaded or loading.
        if self.thumbnails.cache.contains_key(&cache_idx)
            || self.thumbnails.loading.contains(&cache_idx)
        {
            return;
        }

        if self.thumbnails.loading.len() >= self.max_in_flight_thumbnail_requests() {
            return;
        }

        if let Some(wp) = self.cache.wallpapers.get(cache_idx) {
            if let Some(tx) = &self.thumbnails.request_tx {
                let request = ThumbnailRequest {
                    cache_idx,
                    source_path: wp.path.clone(),
                    generation: self.thumbnails.generation,
                };
                if tx.try_send(request).is_ok() {
                    self.thumbnails.loading.insert(cache_idx);
                }
            }
        }
    }

    /// Dynamic max thumbnail cache size based on grid columns.
    /// Capped at 200 to stay well below the Kitty protocol's u8 image ID
    /// limit (255) and prevent ID wrap-around which causes every thumbnail
    /// to render the same picture.
    fn max_thumbnail_cache(&self) -> usize {
        let cols = self.config.thumbnails.grid_columns.max(1);
        (cols * THUMBNAIL_CACHE_MULTIPLIER).clamp(24, 200)
    }

    fn max_in_flight_thumbnail_requests(&self) -> usize {
        let cols = self.config.thumbnails.grid_columns.max(1);
        (cols * THUMBNAIL_MAX_IN_FLIGHT_MULTIPLIER).clamp(6, 12)
    }

    fn new_thumbnail_picker() -> Picker {
        let mut picker = Picker::from_termios().unwrap_or_else(|_| Picker::new((8, 16)));
        picker.guess_protocol();
        picker
    }

    /// Clear in-memory thumbnail state and purge terminal-side image IDs.
    pub(super) fn reset_thumbnail_cache(&mut self) {
        Self::clear_terminal_images();
        self.thumbnails.cache.clear();
        self.thumbnails.cache_order.clear();
        self.thumbnails.loading.clear();
        self.thumbnails.generation = self.thumbnails.generation.wrapping_add(1);

        // Reset picker to avoid long-running image-id wrap behavior.
        if Config::is_kitty_terminal() {
            self.thumbnails.image_picker = Some(Self::new_thumbnail_picker());
        }
    }

    /// Handle a loaded thumbnail from background thread.
    pub fn handle_thumbnail_ready(&mut self, response: ThumbnailResponse) {
        if response.generation != self.thumbnails.generation {
            return;
        }

        self.thumbnails.loading.remove(&response.cache_idx);

        // Ignore stale responses for indices that no longer exist.
        if response.cache_idx >= self.cache.wallpapers.len() {
            return;
        }

        let max_cache = self.max_thumbnail_cache();
        // When cache is full, purge all Kitty images from the terminal
        // and reset the in-memory cache. ratatui-image assigns each
        // protocol a u8 unique_id (0-255) and never sends a delete
        // command when the Rust object is dropped. Without this purge,
        // IDs eventually wrap around and every thumbnail renders the
        // same picture. Visible thumbnails are re-requested on the very
        // next frame from the warm disk cache, so the visual gap is at
        // most one frame.
        if self.thumbnails.cache.len() >= max_cache {
            self.reset_thumbnail_cache();
        }

        if let Some(picker) = &mut self.thumbnails.image_picker {
            let protocol = picker.new_resize_protocol(response.image);
            self.thumbnails.cache.insert(response.cache_idx, protocol);
            self.thumbnails.cache_order.push(response.cache_idx);
        }
    }

    /// Purge all Kitty graphics protocol images from the terminal.
    ///
    /// Sends `APC G a=d,d=A ST` which deletes every stored image and its
    /// placements. Harmlessly ignored by non-Kitty terminals (Sixel, etc.).
    fn clear_terminal_images() {
        if !Config::is_kitty_terminal() {
            return;
        }
        use std::io::Write;
        let _ = std::io::stdout().write_all(b"\x1b_Ga=d,d=A\x1b\\");
        let _ = std::io::stdout().flush();
    }

    /// Check if a thumbnail is ready (also updates LRU order).
    pub fn get_thumbnail(&mut self, cache_idx: usize) -> Option<&mut Box<dyn StatefulProtocol>> {
        if self.thumbnails.cache.contains_key(&cache_idx) {
            // Move to end of LRU order (most recently used).
            if let Some(pos) = self
                .thumbnails
                .cache_order
                .iter()
                .position(|&i| i == cache_idx)
            {
                self.thumbnails.cache_order.remove(pos);
                self.thumbnails.cache_order.push(cache_idx);
            }
        }
        self.thumbnails.cache.get_mut(&cache_idx)
    }

    /// Check if a thumbnail is currently loading.
    pub fn is_loading(&self, cache_idx: usize) -> bool {
        self.thumbnails.loading.contains(&cache_idx)
    }

    /// Set the thumbnail request channel.
    pub fn set_thumb_channel(&mut self, tx: SyncSender<ThumbnailRequest>) {
        self.thumbnails.request_tx = Some(tx);
    }

    /// Handle terminal resize: clear thumbnail cache and re-init picker.
    /// StatefulProtocol objects are sized for the old terminal dimensions
    /// and will render garbled if reused after resize.
    pub fn handle_resize(&mut self) {
        self.reset_thumbnail_cache();

        // Re-detect font metrics for the new terminal size.
        self.thumbnails.image_picker = Some(Self::new_thumbnail_picker());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{FilterState, PairingState, SelectionState, ThumbnailState, UiState};
    use crate::pairing::{PairingHistory, PairingStyleMode};
    use crate::screen::AspectCategory;
    use crate::wallpaper::{Wallpaper, WallpaperCache};
    use std::collections::{HashMap, HashSet};
    use std::path::PathBuf;
    use std::sync::mpsc;

    fn test_wallpaper(name: &str) -> Wallpaper {
        Wallpaper {
            path: PathBuf::from(format!("/tmp/{name}.png")),
            width: 1920,
            height: 1080,
            aspect_category: AspectCategory::Landscape,
            colors: Vec::new(),
            tags: Vec::new(),
            auto_tags: Vec::new(),
            color_weights: Vec::new(),
            embedding: None,
            file_size: 0,
            modified_at: 0,
        }
    }

    fn test_app(wallpaper_count: usize) -> App {
        let wallpapers = (0..wallpaper_count)
            .map(|i| test_wallpaper(&format!("wp-{i}")))
            .collect();

        App {
            screens: Vec::new(),
            cache: WallpaperCache {
                version: 1,
                wallpapers,
                source_dir: PathBuf::from("/tmp"),
                screen_indices: HashMap::new(),
                recursive: false,
            },
            config: Config::default(),
            ui: UiState::default(),
            selection: SelectionState::default(),
            filters: FilterState::default(),
            thumbnails: ThumbnailState {
                image_picker: None,
                cache: HashMap::new(),
                cache_order: Vec::new(),
                loading: HashSet::new(),
                request_tx: None,
                generation: 0,
            },
            pairing: PairingState {
                history: PairingHistory::new(128),
                suggestions: Vec::new(),
                current_wallpapers: HashMap::new(),
                show_preview: false,
                preview_matches: HashMap::new(),
                preview_idx: 0,
                style_mode: PairingStyleMode::default(),
            },
        }
    }

    #[test]
    fn max_thumbnail_cache_is_clamped_to_prevent_id_wrap() {
        let mut app = test_app(0);

        app.config.thumbnails.grid_columns = 1;
        assert_eq!(app.max_thumbnail_cache(), 24);

        app.config.thumbnails.grid_columns = 1_000;
        assert_eq!(app.max_thumbnail_cache(), 200);
    }

    #[test]
    fn in_flight_thumbnail_requests_are_bounded() {
        let mut app = test_app(32);
        app.config.thumbnails.grid_columns = 3;

        let max_in_flight = app.max_in_flight_thumbnail_requests();
        let (tx, _rx) = mpsc::sync_channel(64);
        app.set_thumb_channel(tx);

        for idx in 0..max_in_flight {
            app.request_thumbnail(idx);
        }
        assert_eq!(app.thumbnails.loading.len(), max_in_flight);

        app.request_thumbnail(max_in_flight + 1);
        assert_eq!(app.thumbnails.loading.len(), max_in_flight);
        assert!(!app.is_loading(max_in_flight + 1));
    }

    #[test]
    fn request_thumbnail_does_not_mark_loading_when_queue_is_full() {
        let mut app = test_app(2);
        let (tx, _rx) = mpsc::sync_channel(1);
        app.set_thumb_channel(tx);

        app.request_thumbnail(0);
        assert!(app.is_loading(0));

        app.request_thumbnail(1);
        assert!(!app.is_loading(1));
    }

    #[test]
    fn handle_thumbnail_ready_ignores_stale_generation() {
        let mut app = test_app(1);
        app.thumbnails.generation = 2;
        app.thumbnails.loading.insert(0);

        app.handle_thumbnail_ready(ThumbnailResponse {
            cache_idx: 0,
            image: image::DynamicImage::new_rgba8(1, 1),
            generation: 1,
        });

        assert!(app.is_loading(0));
        assert!(app.thumbnails.cache.is_empty());
    }

    #[test]
    fn handle_thumbnail_ready_current_generation_clears_loading() {
        let mut app = test_app(1);
        app.thumbnails.generation = 7;
        app.thumbnails.loading.insert(0);

        app.handle_thumbnail_ready(ThumbnailResponse {
            cache_idx: 0,
            image: image::DynamicImage::new_rgba8(1, 1),
            generation: 7,
        });

        assert!(!app.is_loading(0));
    }
}
