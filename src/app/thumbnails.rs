use super::{App, Config, ThumbnailRequest, ThumbnailResponse, THUMBNAIL_CACHE_MULTIPLIER};
use ratatui_image::{picker::Picker, protocol::StatefulProtocol};
use std::sync::mpsc::SyncSender;

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

    /// Clear in-memory thumbnail state and purge terminal-side image IDs.
    pub(super) fn reset_thumbnail_cache(&mut self) {
        Self::clear_terminal_images();
        self.thumbnails.cache.clear();
        self.thumbnails.cache_order.clear();
        self.thumbnails.loading.clear();
        self.thumbnails.generation = self.thumbnails.generation.wrapping_add(1);

        // Reset picker to avoid long-running image-id wrap behavior.
        if Config::is_kitty_terminal() {
            if let Ok(mut picker) = Picker::from_termios() {
                picker.guess_protocol();
                self.thumbnails.image_picker = Some(picker);
            }
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
        if let Ok(mut picker) = Picker::from_termios() {
            picker.guess_protocol();
            self.thumbnails.image_picker = Some(picker);
        }
    }
}
