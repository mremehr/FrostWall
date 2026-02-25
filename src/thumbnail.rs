use anyhow::{Context, Result};
use fast_image_resize::{images::Image, ResizeOptions, Resizer};
use image::{DynamicImage, RgbaImage};
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

// Higher resolution for crisp thumbnails
pub const THUMB_WIDTH: u32 = 800;
pub const THUMB_HEIGHT: u32 = 600;

// JPEG quality (0-100) - 92 is high quality with good compression
const JPEG_QUALITY: u8 = 92;
const UNSHARP_SIGMA: f32 = 0.5;
const UNSHARP_THRESHOLD: i32 = 1;

pub struct ThumbnailCache {
    cache_dir: PathBuf,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        let cache_dir = directories::ProjectDirs::from("com", "mrmattias", "frostwall")
            .map(|dirs| dirs.cache_dir().to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/tmp/frostwall"))
            .join("thumbs_v2"); // New version for higher quality

        // Ensure cache directory exists
        let _ = fs::create_dir_all(&cache_dir);

        Self { cache_dir }
    }

    /// Generate a hash-based filename for the thumbnail
    fn thumb_filename(&self, source_path: &Path) -> PathBuf {
        let mut hasher = DefaultHasher::new();
        source_path.to_string_lossy().hash(&mut hasher);

        // Include modification time in hash if available
        if let Ok(metadata) = fs::metadata(source_path) {
            if let Ok(modified) = metadata.modified() {
                modified.hash(&mut hasher);
            }
        }

        let hash = hasher.finish();
        self.cache_dir.join(format!("{:016x}.jpg", hash))
    }

    /// Check if a cached thumbnail exists and is valid
    pub fn get_cached(&self, source_path: &Path) -> Option<PathBuf> {
        let thumb_path = self.thumb_filename(source_path);
        if thumb_path.exists() {
            Some(thumb_path)
        } else {
            None
        }
    }

    /// Load a thumbnail as DynamicImage (for ratatui-image)
    pub fn load(&self, source_path: &Path) -> Result<DynamicImage> {
        if let Some(thumb_path) = self.get_cached(source_path) {
            match image::open(&thumb_path) {
                Ok(img) => return Ok(img),
                Err(err) => {
                    // Corrupted cache entry: regenerate from source below.
                    eprintln!(
                        "Warning: failed to decode cached thumbnail {}: {}",
                        thumb_path.display(),
                        err
                    );
                    let _ = fs::remove_file(&thumb_path);
                }
            }
        }

        let thumb_path = self.thumb_filename(source_path);
        let result_image = Self::build_thumbnail_image(source_path)?;
        if let Err(err) = save_as_jpeg(&result_image, &thumb_path, JPEG_QUALITY) {
            // Rendering should still work even if the cache cannot be written.
            eprintln!(
                "Warning: failed to persist thumbnail {}: {}",
                thumb_path.display(),
                err
            );
        }

        Ok(DynamicImage::ImageRgba8(result_image))
    }

    /// Calculate dimensions that fit within bounds while maintaining aspect ratio
    fn fit_dimensions(src_w: u32, src_h: u32, max_w: u32, max_h: u32) -> (u32, u32) {
        if src_w == 0 || src_h == 0 {
            return (max_w.max(1), max_h.max(1));
        }
        let ratio_w = max_w as f32 / src_w as f32;
        let ratio_h = max_h as f32 / src_h as f32;
        let ratio = ratio_w.min(ratio_h);

        let dst_w = (src_w as f32 * ratio).round() as u32;
        let dst_h = (src_h as f32 * ratio).round() as u32;

        (dst_w.max(1), dst_h.max(1))
    }

    fn build_thumbnail_image(source_path: &Path) -> Result<RgbaImage> {
        let src_image = image::open(source_path)
            .with_context(|| format!("Failed to open image: {}", source_path.display()))?;

        let src_rgba = src_image.to_rgba8();
        let (src_width, src_height) = (src_rgba.width(), src_rgba.height());
        let (dst_width, dst_height) =
            Self::fit_dimensions(src_width, src_height, THUMB_WIDTH, THUMB_HEIGHT);

        let src_fir = Image::from_vec_u8(
            src_width,
            src_height,
            src_rgba.into_raw(),
            fast_image_resize::PixelType::U8x4,
        )?;

        let mut dst_fir = Image::new(dst_width, dst_height, fast_image_resize::PixelType::U8x4);

        let mut resizer = Resizer::new();
        resizer.resize(
            &src_fir,
            &mut dst_fir,
            &ResizeOptions::new().resize_alg(fast_image_resize::ResizeAlg::Convolution(
                fast_image_resize::FilterType::Lanczos3,
            )),
        )?;

        let dst_buffer = dst_fir.into_vec();
        let result_image = RgbaImage::from_raw(dst_width, dst_height, dst_buffer)
            .context("Failed to create output image")?;

        // Library implementation is simpler to maintain than the old handwritten blur/sharpen.
        Ok(image::imageops::unsharpen(
            &result_image,
            UNSHARP_SIGMA,
            UNSHARP_THRESHOLD,
        ))
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Save RGBA image as JPEG with specified quality
fn save_as_jpeg(img: &RgbaImage, path: &Path, quality: u8) -> Result<()> {
    // Convert RGBA to RGB for JPEG
    let (width, height) = (img.width(), img.height());
    let mut rgb_data = Vec::with_capacity((width * height * 3) as usize);

    for pixel in img.pixels() {
        rgb_data.push(pixel[0]); // R
        rgb_data.push(pixel[1]); // G
        rgb_data.push(pixel[2]); // B
    }

    let rgb_img =
        image::RgbImage::from_raw(width, height, rgb_data).context("Failed to create RGB image")?;

    let file =
        File::create(path).with_context(|| format!("Failed to create file: {}", path.display()))?;
    let writer = BufWriter::new(file);

    let mut encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(writer, quality);
    encoder
        .encode_image(&rgb_img)
        .with_context(|| format!("Failed to encode JPEG: {}", path.display()))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{Rgb, RgbImage};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_tmp_dir() -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock before epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("frostwall-thumb-test-{nanos}"))
    }

    #[test]
    fn fit_dimensions_preserves_aspect_ratio() {
        assert_eq!(
            ThumbnailCache::fit_dimensions(3840, 2160, 800, 600),
            (800, 450)
        );
        assert_eq!(
            ThumbnailCache::fit_dimensions(1080, 1920, 800, 600),
            (338, 600)
        );
    }

    #[test]
    fn load_regenerates_corrupted_cached_thumbnail() -> Result<()> {
        let root = unique_tmp_dir();
        let src_dir = root.join("src");
        let cache_dir = root.join("cache");
        fs::create_dir_all(&src_dir)?;
        fs::create_dir_all(&cache_dir)?;

        let source_path = src_dir.join("image.png");
        let source = RgbImage::from_pixel(64, 64, Rgb([240, 80, 80]));
        source.save(&source_path)?;

        let cache = ThumbnailCache {
            cache_dir: cache_dir.clone(),
        };
        let thumb_path = cache.thumb_filename(&source_path);
        fs::write(&thumb_path, b"not-a-valid-jpeg")?;

        let loaded = cache.load(&source_path)?;
        let (expected_w, expected_h) =
            ThumbnailCache::fit_dimensions(64, 64, THUMB_WIDTH, THUMB_HEIGHT);
        assert_eq!(loaded.width(), expected_w);
        assert_eq!(loaded.height(), expected_h);
        assert!(image::open(&thumb_path).is_ok());

        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}
