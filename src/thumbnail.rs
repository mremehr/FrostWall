use anyhow::{Context, Result};
use fast_image_resize::{images::Image, ResizeOptions, Resizer};
use image::{DynamicImage, ImageBuffer, Rgba, RgbaImage};
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

    /// Generate a thumbnail using fast_image_resize (SIMD accelerated)
    pub fn generate(&self, source_path: &Path) -> Result<PathBuf> {
        let thumb_path = self.thumb_filename(source_path);

        // Return cached if exists
        if thumb_path.exists() {
            return Ok(thumb_path);
        }

        // Load source image
        let src_image = image::open(source_path)
            .with_context(|| format!("Failed to open image: {}", source_path.display()))?;

        // Convert to RGBA8
        let src_rgba = src_image.to_rgba8();
        let (src_width, src_height) = (src_rgba.width(), src_rgba.height());

        // Calculate target dimensions maintaining aspect ratio
        let (dst_width, dst_height) =
            Self::fit_dimensions(src_width, src_height, THUMB_WIDTH, THUMB_HEIGHT);

        // Create fast_image_resize source image
        let src_fir = Image::from_vec_u8(
            src_width,
            src_height,
            src_rgba.into_raw(),
            fast_image_resize::PixelType::U8x4,
        )?;

        // Create destination image
        let mut dst_fir = Image::new(dst_width, dst_height, fast_image_resize::PixelType::U8x4);

        // Resize using SIMD-accelerated Lanczos3 (high quality)
        let mut resizer = Resizer::new();
        resizer.resize(
            &src_fir,
            &mut dst_fir,
            &ResizeOptions::new().resize_alg(fast_image_resize::ResizeAlg::Convolution(
                fast_image_resize::FilterType::Lanczos3,
            )),
        )?;

        // Convert back to image crate format
        let dst_buffer = dst_fir.into_vec();
        let mut result_image = RgbaImage::from_raw(dst_width, dst_height, dst_buffer)
            .context("Failed to create output image")?;

        // Apply unsharp mask for crispness
        apply_unsharp_mask(&mut result_image, 0.5, 1.0);

        // Save as high-quality JPEG
        save_as_jpeg(&result_image, &thumb_path, JPEG_QUALITY)?;

        Ok(thumb_path)
    }

    /// Load a thumbnail as DynamicImage (for ratatui-image)
    pub fn load(&self, source_path: &Path) -> Result<DynamicImage> {
        // Try cached first
        if let Some(thumb_path) = self.get_cached(source_path) {
            return image::open(&thumb_path).with_context(|| {
                format!("Failed to load cached thumbnail: {}", thumb_path.display())
            });
        }

        // Generate and load
        let thumb_path = self.generate(source_path)?;
        image::open(&thumb_path)
            .with_context(|| format!("Failed to load thumbnail: {}", thumb_path.display()))
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

    /// Clear the thumbnail cache
    #[allow(dead_code)]
    pub fn clear(&self) -> Result<()> {
        if self.cache_dir.exists() {
            fs::remove_dir_all(&self.cache_dir)?;
            fs::create_dir_all(&self.cache_dir)?;
        }
        Ok(())
    }

    /// Get cache directory path
    #[allow(dead_code)]
    pub fn cache_dir(&self) -> &Path {
        &self.cache_dir
    }
}

impl Default for ThumbnailCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply unsharp mask to sharpen the image
/// radius: blur radius (0.5-2.0 typical)
/// amount: sharpening strength (0.5-2.0 typical)
fn apply_unsharp_mask(img: &mut RgbaImage, radius: f32, amount: f32) {
    let (width, height) = (img.width(), img.height());

    // Create a blurred copy using box blur (fast approximation)
    let blurred = box_blur(img, radius as u32 + 1);

    // Apply unsharp mask: result = original + amount * (original - blurred)
    for y in 0..height {
        for x in 0..width {
            let orig = img.get_pixel(x, y);
            let blur = blurred.get_pixel(x, y);

            let mut new_pixel = [0u8; 4];
            for c in 0..3 {
                let diff = orig[c] as f32 - blur[c] as f32;
                let sharpened = orig[c] as f32 + amount * diff;
                new_pixel[c] = sharpened.clamp(0.0, 255.0) as u8;
            }
            new_pixel[3] = orig[3]; // Keep alpha unchanged

            img.put_pixel(x, y, Rgba(new_pixel));
        }
    }
}

/// Separable box blur (two-pass) for unsharp mask
fn box_blur(img: &RgbaImage, radius: u32) -> RgbaImage {
    let (width, height) = (img.width(), img.height());
    let r = radius as i32;

    // Horizontal pass: read from img, write to temp
    let mut temp: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 4];
            let mut count = 0u32;

            for dx in -r..=r {
                let nx = (x as i32 + dx).clamp(0, width as i32 - 1) as u32;
                let pixel = img.get_pixel(nx, y);
                for c in 0..4 {
                    sum[c] += pixel[c] as u32;
                }
                count += 1;
            }

            let mut avg = [0u8; 4];
            for c in 0..4 {
                avg[c] = (sum[c] / count) as u8;
            }
            temp.put_pixel(x, y, Rgba(avg));
        }
    }

    // Vertical pass: read from temp, write to result
    let mut result: ImageBuffer<Rgba<u8>, Vec<u8>> = ImageBuffer::new(width, height);
    for y in 0..height {
        for x in 0..width {
            let mut sum = [0u32; 4];
            let mut count = 0u32;

            for dy in -r..=r {
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;
                let pixel = temp.get_pixel(x, ny);
                for c in 0..4 {
                    sum[c] += pixel[c] as u32;
                }
                count += 1;
            }

            let mut avg = [0u8; 4];
            for c in 0..4 {
                avg[c] = (sum[c] / count) as u8;
            }
            result.put_pixel(x, y, Rgba(avg));
        }
    }

    result
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
