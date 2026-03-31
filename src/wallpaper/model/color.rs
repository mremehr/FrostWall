#[cfg(feature = "clip")]
use crate::clip::AutoTag;
use crate::wallpaper::Wallpaper;
use anyhow::{Context, Result};
use image::imageops::FilterType;
use kmeans_colors::get_kmeans_hamerly;
use palette::{IntoColor, Lab, Srgb};

impl Wallpaper {
    /// Extract colors for a wallpaper (call after from_path_fast if colors needed).
    pub fn extract_colors(&mut self) -> Result<()> {
        if !self.colors.is_empty() {
            return Ok(());
        }

        const K: usize = 5;
        const CONVERGENCE_THRESHOLD: f32 = 5.0;
        const MAX_ITERATIONS: u32 = 30;
        const THUMBNAIL_SIZE: u32 = 64;

        let image = image::open(&self.path).context("Failed to open image")?;
        let thumbnail = image.resize(THUMBNAIL_SIZE, THUMBNAIL_SIZE, FilterType::Triangle);
        let thumbnail_rgb = thumbnail.to_rgb8();
        let mut lab_colors: Vec<Lab> =
            Vec::with_capacity((thumbnail_rgb.width() * thumbnail_rgb.height()) as usize);
        lab_colors.extend(thumbnail_rgb.pixels().map(|pixel| {
            let rgb = Srgb::new(
                pixel.0[0] as f32 / 255.0,
                pixel.0[1] as f32 / 255.0,
                pixel.0[2] as f32 / 255.0,
            );
            let lab_color: Lab = rgb.into_color();
            lab_color
        }));

        let result = get_kmeans_hamerly(
            K,
            MAX_ITERATIONS as usize,
            CONVERGENCE_THRESHOLD,
            false,
            &lab_colors,
            0,
        );

        let total_pixels = lab_colors.len() as f32;
        let mut counts = [0usize; K];
        for &idx in &result.indices {
            counts[idx as usize] += 1;
        }

        let mut color_weight_pairs: Vec<(String, f32)> = result
            .centroids
            .iter()
            .zip(counts.iter())
            .map(|(centroid, &count)| {
                let rgb: Srgb = (*centroid).into_color();
                let r = (rgb.red * 255.0) as u8;
                let g = (rgb.green * 255.0) as u8;
                let b = (rgb.blue * 255.0) as u8;
                let hex = format!("#{:02x}{:02x}{:02x}", r, g, b);
                let weight = count as f32 / total_pixels;
                (hex, weight)
            })
            .collect();

        color_weight_pairs.sort_by(|left, right| {
            right
                .1
                .partial_cmp(&left.1)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        self.colors = color_weight_pairs
            .iter()
            .map(|(color, _)| color.clone())
            .collect();
        self.color_weights = color_weight_pairs
            .iter()
            .map(|(_, weight)| *weight)
            .collect();

        Ok(())
    }

    /// Get primary/dominant color (first in list).
    #[cfg(test)]
    pub fn primary_color(&self) -> Option<&str> {
        self.colors.first().map(|color| color.as_str())
    }

    /// Set auto tags (replaces existing).
    #[cfg(feature = "clip")]
    pub fn set_auto_tags(&mut self, tags: Vec<AutoTag>) {
        self.auto_tags = tags;
    }

    /// Set embedding (replaces existing).
    #[cfg(feature = "clip")]
    pub fn set_embedding(&mut self, embedding: Vec<f32>) {
        self.embedding = Some(embedding);
    }
}
