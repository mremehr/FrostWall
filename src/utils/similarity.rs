use super::color::{color_brightness, color_saturation, color_similarity};
use serde::{Deserialize, Serialize};

fn uniform_weights(len: usize) -> Vec<f32> {
    if len == 0 {
        return Vec::new();
    }
    vec![1.0 / len as f32; len]
}

fn normalized_weights(len: usize, weights: &[f32]) -> Vec<f32> {
    if len == 0 {
        return Vec::new();
    }

    let mut normalized = vec![0.0; len];
    for (i, value) in normalized.iter_mut().enumerate() {
        *value = weights.get(i).copied().unwrap_or(0.0);
    }

    let sum: f32 = normalized.iter().sum();
    if sum > 0.0 {
        for value in &mut normalized {
            *value /= sum;
        }
        normalized
    } else {
        uniform_weights(len)
    }
}

/// Precomputed palette features used to speed up similarity comparisons.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PaletteProfile {
    /// Normalized per-color weights (sum ~= 1.0)
    #[serde(default)]
    pub normalized_weights: Vec<f32>,
    /// Weighted average perceived brightness for the palette (0.0-1.0)
    #[serde(default)]
    pub weighted_brightness: f32,
    /// Weighted average saturation for the palette (0.0-1.0)
    #[serde(default)]
    pub weighted_saturation: f32,
}

/// Build a precomputed palette profile from colors and optional weights.
pub fn build_palette_profile(colors: &[String], weights: &[f32]) -> PaletteProfile {
    if colors.is_empty() {
        return PaletteProfile::default();
    }

    let normalized_weights = normalized_weights(colors.len(), weights);

    let weighted_brightness: f32 = colors
        .iter()
        .zip(normalized_weights.iter())
        .map(|(c, w)| color_brightness(c) * w)
        .sum();

    let weighted_saturation: f32 = colors
        .iter()
        .zip(normalized_weights.iter())
        .map(|(c, w)| color_saturation(c) * w)
        .sum();

    PaletteProfile {
        normalized_weights,
        weighted_brightness,
        weighted_saturation,
    }
}

fn palette_similarity_normalized(
    colors1: &[String],
    norm_weights1: &[f32],
    colors2: &[String],
    norm_weights2: &[f32],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    let mut total_similarity = 0.0;

    // For each color in palette 1, find best match in palette 2.
    for (i, c1) in colors1.iter().enumerate() {
        let w1 = norm_weights1.get(i).copied().unwrap_or(0.0);
        if w1 < 0.01 {
            continue; // Skip very minor colors.
        }

        let mut best_sim = 0.0;

        for (j, c2) in colors2.iter().enumerate() {
            let w2 = norm_weights2.get(j).copied().unwrap_or(0.0);
            let sim = color_similarity(c1, c2);

            // Boost similarity when matching dominant colors with dominant colors.
            let weight_boost = (w2 * 2.0).min(1.0);
            let boosted_sim = sim * (0.7 + 0.3 * weight_boost);

            if boosted_sim > best_sim {
                best_sim = boosted_sim;
            }
        }

        total_similarity += best_sim * w1;
    }

    total_similarity
}

/// Find the best color match between two palettes, weighted by color dominance.
///
/// Returns a weighted similarity score (0.0-1.0).
#[cfg_attr(not(test), allow(dead_code))]
pub fn palette_similarity_weighted(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    let profile1 = build_palette_profile(colors1, weights1);
    let profile2 = build_palette_profile(colors2, weights2);

    palette_similarity_normalized(
        colors1,
        &profile1.normalized_weights,
        colors2,
        &profile2.normalized_weights,
    )
}

/// Calculate overall image similarity based on color profile.
/// Returns a score from 0.0 (very different) to 1.0 (very similar).
#[cfg_attr(not(test), allow(dead_code))]
pub fn image_similarity(colors1: &[String], colors2: &[String]) -> f32 {
    image_similarity_weighted(colors1, &[], colors2, &[])
}

/// Calculate overall image similarity based on color profile with weights.
/// Returns a score from 0.0 (very different) to 1.0 (very similar).
pub fn image_similarity_weighted(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    let profile1 = build_palette_profile(colors1, weights1);
    let profile2 = build_palette_profile(colors2, weights2);

    image_similarity_with_profiles(colors1, &profile1, colors2, &profile2)
}

/// Calculate image similarity from precomputed palette profiles.
pub fn image_similarity_with_profiles(
    colors1: &[String],
    profile1: &PaletteProfile,
    colors2: &[String],
    profile2: &PaletteProfile,
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    let color_sim = palette_similarity_normalized(
        colors1,
        &profile1.normalized_weights,
        colors2,
        &profile2.normalized_weights,
    );

    let bright_sim = 1.0 - (profile1.weighted_brightness - profile2.weighted_brightness).abs();
    let sat_sim = 1.0 - (profile1.weighted_saturation - profile2.weighted_saturation).abs();

    color_sim * 0.6 + bright_sim * 0.25 + sat_sim * 0.15
}

/// Find similar wallpapers from an iterator of candidates.
///
/// Keeps only top-k matches while iterating, so callers can avoid collecting
/// large temporary vectors of candidates.
#[cfg_attr(not(test), allow(dead_code))]
pub fn find_similar_wallpapers_iter<'a, I>(
    target_colors: &[String],
    all_wallpapers: I,
    limit: usize,
) -> Vec<(f32, usize)>
where
    I: IntoIterator<Item = (usize, &'a [String])>,
{
    if limit == 0 {
        return Vec::new();
    }

    let target_profile = build_palette_profile(target_colors, &[]);
    let mut top: Vec<(f32, usize)> = Vec::with_capacity(limit);

    for (idx, colors) in all_wallpapers {
        if colors.is_empty() {
            continue;
        }

        let candidate_profile = build_palette_profile(colors, &[]);
        let sim = image_similarity_with_profiles(
            target_colors,
            &target_profile,
            colors,
            &candidate_profile,
        );

        if top.len() < limit {
            top.push((sim, idx));
            continue;
        }

        let mut min_idx = 0;
        for i in 1..top.len() {
            if top[i].0 < top[min_idx].0 {
                min_idx = i;
            }
        }

        if sim > top[min_idx].0 {
            top[min_idx] = (sim, idx);
        }
    }

    top.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    top
}

/// Find similar wallpapers using precomputed palette profiles.
pub fn find_similar_wallpapers_with_profiles_iter<'a, I>(
    target_colors: &[String],
    target_profile: &PaletteProfile,
    all_wallpapers: I,
    limit: usize,
) -> Vec<(f32, usize)>
where
    I: IntoIterator<Item = (usize, &'a [String], &'a PaletteProfile)>,
{
    if limit == 0 {
        return Vec::new();
    }

    let mut top: Vec<(f32, usize)> = Vec::with_capacity(limit);

    for (idx, colors, profile) in all_wallpapers {
        if colors.is_empty() {
            continue;
        }

        let sim = image_similarity_with_profiles(target_colors, target_profile, colors, profile);

        if top.len() < limit {
            top.push((sim, idx));
            continue;
        }

        let mut min_idx = 0;
        for i in 1..top.len() {
            if top[i].0 < top[min_idx].0 {
                min_idx = i;
            }
        }

        if sim > top[min_idx].0 {
            top[min_idx] = (sim, idx);
        }
    }

    top.sort_unstable_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    top
}
