use crate::utils::project_cache_dir;
use anyhow::{Context, Result};
use fast_image_resize::{images::Image, ResizeOptions, Resizer};
use image::{DynamicImage, RgbaImage};
use std::collections::hash_map::DefaultHasher;
use std::fs::{self, File};
use std::hash::{Hash, Hasher};
use std::io::BufWriter;
use std::path::{Path, PathBuf};

// Resolution tuned for carousel slots (~300-500 px wide).
// Generating 2560 px images for 300 px display was ~9× wasted work.
pub const THUMB_WIDTH: u32 = 800;
pub const THUMB_HEIGHT: u32 = 600;
pub const MIN_THUMB_WIDTH: u32 = 320;
pub const MIN_THUMB_HEIGHT: u32 = 240;

// JPEG quality (0-100) - 92 is high quality with good compression
const JPEG_QUALITY: u8 = 92;
const UNSHARP_SIGMA: f32 = 0.5;
const UNSHARP_THRESHOLD: i32 = 1;
#[cfg(any(test, feature = "clip"))]
const THUMBNAIL_CACHE_PREFIX: &str = "thumbs_v";
const THUMBNAIL_CACHE_VERSION: u32 = 3;

pub struct ThumbnailCache {
    cache_dir: PathBuf,
    width: u32,
    height: u32,
    quality: u8,
}

impl ThumbnailCache {
    pub fn new() -> Self {
        Self::new_with_settings(THUMB_WIDTH, THUMB_HEIGHT, JPEG_QUALITY)
    }

    pub fn new_with_settings(width: u32, height: u32, quality: u8) -> Self {
        let (width, height) = effective_thumbnail_bounds(width, height);
        let quality = quality.clamp(1, 100);
        let cache_dir = Self::cache_root().join(Self::cache_dir_name(width, height, quality));

        // Ensure cache directory exists
        let _ = fs::create_dir_all(&cache_dir);

        Self {
            cache_dir,
            width,
            height,
            quality,
        }
    }

    fn cache_root() -> PathBuf {
        project_cache_dir(PathBuf::from("/tmp/frostwall"))
    }

    fn cache_dir_name(width: u32, height: u32, quality: u8) -> String {
        format!(
            "thumbs_v{}_{}x{}_q{}",
            THUMBNAIL_CACHE_VERSION, width, height, quality
        )
    }

    fn thumb_file_name(source_path: &Path) -> String {
        let mut hasher = DefaultHasher::new();
        source_path.to_string_lossy().hash(&mut hasher);

        // Include modification time in hash if available
        if let Ok(metadata) = fs::metadata(source_path) {
            if let Ok(modified) = metadata.modified() {
                modified.hash(&mut hasher);
            }
        }

        format!("{:016x}.jpg", hasher.finish())
    }

    /// Generate a hash-based filename for the thumbnail
    fn thumb_filename(&self, source_path: &Path) -> PathBuf {
        self.cache_dir.join(Self::thumb_file_name(source_path))
    }

    #[cfg(any(test, feature = "clip"))]
    fn cache_variant_dirs(root: &Path) -> Vec<PathBuf> {
        let mut variant_dirs: Vec<PathBuf> = match fs::read_dir(root) {
            Ok(entries) => entries
                .filter_map(|entry| entry.ok())
                .map(|entry| entry.path())
                .filter(|path| {
                    path.is_dir()
                        && path
                            .file_name()
                            .and_then(|name| name.to_str())
                            .is_some_and(|name| name.starts_with(THUMBNAIL_CACHE_PREFIX))
                })
                .collect(),
            Err(_) => Vec::new(),
        };

        variant_dirs.sort_by(|a, b| {
            let a_name = a
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let b_name = b
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or_default();
            let a_version = parse_thumbnail_cache_version(a_name);
            let b_version = parse_thumbnail_cache_version(b_name);
            b_version.cmp(&a_version).then_with(|| b_name.cmp(a_name))
        });
        variant_dirs
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
        let result_image = self.build_thumbnail_image(source_path)?;
        if let Err(err) = save_as_jpeg(&result_image, &thumb_path, self.quality) {
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

    fn build_thumbnail_image(&self, source_path: &Path) -> Result<RgbaImage> {
        let src_image = image::open(source_path)
            .with_context(|| format!("Failed to open image: {}", source_path.display()))?;

        let src_rgba = src_image.to_rgba8();
        let (src_width, src_height) = (src_rgba.width(), src_rgba.height());
        let (dst_width, dst_height) =
            Self::fit_dimensions(src_width, src_height, self.width, self.height);

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

#[cfg(any(test, feature = "clip"))]
fn parse_thumbnail_cache_version(dir_name: &str) -> u32 {
    dir_name
        .strip_prefix(THUMBNAIL_CACHE_PREFIX)
        .map(|suffix| {
            suffix
                .chars()
                .take_while(|ch| ch.is_ascii_digit())
                .collect::<String>()
        })
        .and_then(|digits| digits.parse().ok())
        .unwrap_or(0)
}

#[cfg(any(test, feature = "clip"))]
pub struct ThumbnailLookup {
    candidate_dirs: Vec<PathBuf>,
}

#[cfg(any(test, feature = "clip"))]
impl ThumbnailLookup {
    #[cfg(feature = "clip")]
    pub fn new(width: u32, height: u32, quality: u8) -> Self {
        Self::with_root(ThumbnailCache::cache_root(), width, height, quality)
    }

    fn with_root(root: PathBuf, width: u32, height: u32, quality: u8) -> Self {
        let (width, height) = effective_thumbnail_bounds(width, height);
        let quality = quality.clamp(1, 100);
        let preferred = root.join(ThumbnailCache::cache_dir_name(width, height, quality));
        let mut candidate_dirs = vec![preferred.clone()];
        candidate_dirs.extend(
            ThumbnailCache::cache_variant_dirs(&root)
                .into_iter()
                .filter(|path| path != &preferred),
        );
        Self { candidate_dirs }
    }

    pub fn find(&self, source_path: &Path) -> Option<PathBuf> {
        let file_name = ThumbnailCache::thumb_file_name(source_path);
        self.candidate_dirs
            .iter()
            .map(|dir| dir.join(&file_name))
            .find(|path| path.exists())
    }
}

/// Clamp thumbnail bounds to a quality floor that still allows large carousel tiles to scale well.
pub fn effective_thumbnail_bounds(width: u32, height: u32) -> (u32, u32) {
    (
        width.max(MIN_THUMB_WIDTH).max(32),
        height.max(MIN_THUMB_HEIGHT).max(32),
    )
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
            width: THUMB_WIDTH,
            height: THUMB_HEIGHT,
            quality: JPEG_QUALITY,
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

    #[test]
    fn find_any_cached_prefers_newer_variant_dirs() -> Result<()> {
        let root = unique_tmp_dir();
        let src_dir = root.join("src");
        fs::create_dir_all(&src_dir)?;

        let source_path = src_dir.join("image.png");
        let source = RgbImage::from_pixel(32, 32, Rgb([80, 120, 240]));
        source.save(&source_path)?;

        let legacy_dir = root.join("thumbs_v2");
        let current_dir = root.join("thumbs_v3_800x600_q92");
        fs::create_dir_all(&legacy_dir)?;
        fs::create_dir_all(&current_dir)?;

        let file_name = ThumbnailCache::thumb_file_name(&source_path);
        let legacy_path = legacy_dir.join(&file_name);
        let current_path = current_dir.join(&file_name);
        fs::write(&legacy_path, b"legacy")?;
        fs::write(&current_path, b"current")?;

        let lookup =
            ThumbnailLookup::with_root(root.clone(), THUMB_WIDTH, THUMB_HEIGHT, JPEG_QUALITY);
        assert_eq!(lookup.find(&source_path), Some(current_path));

        let _ = fs::remove_dir_all(root);
        Ok(())
    }
}
