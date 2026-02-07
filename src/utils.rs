use palette::{IntoColor, Lab, Srgb};
use std::path::Path;

/// Supported image file extensions
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

/// Types of color harmony between two palettes
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColorHarmony {
    /// Colors are very similar (within 30°)
    Analogous,
    /// Colors are opposite on the color wheel (180° ± 15°)
    Complementary,
    /// Colors form a triadic harmony (120° apart)
    Triadic,
    /// Complement + adjacent colors (150-180°)
    SplitComplementary,
    /// No specific harmony detected
    None,
}

impl ColorHarmony {
    /// Get a display name for the harmony type
    #[allow(dead_code)]
    pub fn name(&self) -> &'static str {
        match self {
            ColorHarmony::Analogous => "Analogous",
            ColorHarmony::Complementary => "Complementary",
            ColorHarmony::Triadic => "Triadic",
            ColorHarmony::SplitComplementary => "Split-Complementary",
            ColorHarmony::None => "None",
        }
    }

    /// Get the bonus multiplier for this harmony type
    pub fn bonus(&self) -> f32 {
        match self {
            ColorHarmony::Analogous => 1.0,     // Similar colors always work
            ColorHarmony::Complementary => 0.9, // Strong contrast, usually works
            ColorHarmony::Triadic => 0.7,       // Balanced but can be busy
            ColorHarmony::SplitComplementary => 0.8,
            ColorHarmony::None => 0.0,
        }
    }
}

/// Parse hex color string to RGB tuple
/// Supports "#RRGGBB" and "RRGGBB" formats
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
    let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
    let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Convert hex color to LAB color space
pub fn hex_to_lab(hex: &str) -> Option<Lab> {
    let (r, g, b) = hex_to_rgb(hex)?;
    let rgb = Srgb::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0);
    Some(rgb.into_color())
}

/// Convert hex color to HSL and return hue (0-360), saturation (0-1), lightness (0-1)
pub fn hex_to_hsl(hex: &str) -> Option<(f32, f32, f32)> {
    let (r, g, b) = hex_to_rgb(hex)?;
    let r = r as f32 / 255.0;
    let g = g as f32 / 255.0;
    let b = b as f32 / 255.0;

    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let lightness = (max + min) / 2.0;

    if delta < 0.0001 {
        // Achromatic (gray)
        return Some((0.0, 0.0, lightness));
    }

    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let hue = if (max - r).abs() < 0.0001 {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < 0.0001 {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let hue = if hue < 0.0 { hue + 360.0 } else { hue };

    Some((hue, saturation, lightness))
}

/// Calculate the angular difference between two hue values (0-180)
fn hue_difference(h1: f32, h2: f32) -> f32 {
    let diff = (h1 - h2).abs();
    if diff > 180.0 {
        360.0 - diff
    } else {
        diff
    }
}

/// Detect the color harmony between two palettes
/// Returns the harmony type and a strength score (0.0-1.0)
pub fn detect_harmony(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> (ColorHarmony, f32) {
    if colors1.is_empty() || colors2.is_empty() {
        return (ColorHarmony::None, 0.0);
    }

    // Get the dominant (highest weight) saturated color from each palette
    let get_dominant_hue = |colors: &[String], weights: &[f32]| -> Option<(f32, f32)> {
        colors
            .iter()
            .zip(
                weights
                    .iter()
                    .chain(std::iter::repeat(&(1.0 / colors.len() as f32))),
            )
            .filter_map(|(c, w)| {
                hex_to_hsl(c).and_then(|(h, s, l)| {
                    // Only consider colors with enough saturation
                    if s > 0.15 && l > 0.1 && l < 0.9 {
                        Some((h, s * w))
                    } else {
                        None
                    }
                })
            })
            .max_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
            .map(|(h, _)| (h, 1.0))
    };

    let hue1 = get_dominant_hue(colors1, weights1);
    let hue2 = get_dominant_hue(colors2, weights2);

    match (hue1, hue2) {
        (Some((h1, _)), Some((h2, _))) => {
            let diff = hue_difference(h1, h2);

            // Determine harmony type based on hue difference
            let (harmony, strength) = if diff < 30.0 {
                // Analogous: similar hues
                (ColorHarmony::Analogous, 1.0 - diff / 30.0)
            } else if (165.0..=195.0).contains(&diff) {
                // Complementary: opposite hues
                let center_diff = (diff - 180.0).abs();
                (ColorHarmony::Complementary, 1.0 - center_diff / 15.0)
            } else if (105.0..=135.0).contains(&diff) {
                // Triadic: 120° apart
                let center_diff = (diff - 120.0).abs();
                (ColorHarmony::Triadic, 1.0 - center_diff / 15.0)
            } else if (135.0..165.0).contains(&diff) {
                // Split-complementary
                let center_diff = (diff - 150.0).abs();
                (ColorHarmony::SplitComplementary, 1.0 - center_diff / 15.0)
            } else {
                (ColorHarmony::None, 0.0)
            };

            (harmony, strength.max(0.0))
        }
        _ => {
            // One or both palettes are achromatic - check brightness match instead
            (ColorHarmony::None, 0.0)
        }
    }
}

/// Calculate Delta E (CIE76) color distance between two LAB colors
/// Lower values = more similar, 0 = identical
/// < 1.0: Not perceptible by human eye
/// 1-2: Perceptible through close observation
/// 2-10: Perceptible at a glance
/// Calculate Delta E 2000 (CIEDE2000) - perceptually uniform color difference
///
/// This is more accurate than CIE76, especially for:
/// - Dark colors
/// - Saturated colors
/// - Neutral/gray colors
///
/// Reference: https://en.wikipedia.org/wiki/Color_difference#CIEDE2000
pub fn delta_e_2000(lab1: &Lab, lab2: &Lab) -> f32 {
    use std::f32::consts::PI;

    let l1 = lab1.l;
    let a1 = lab1.a;
    let b1 = lab1.b;
    let l2 = lab2.l;
    let a2 = lab2.a;
    let b2 = lab2.b;

    // Weighting factors (standard values)
    let k_l = 1.0_f32;
    let k_c = 1.0_f32;
    let k_h = 1.0_f32;

    // Calculate C'ab (chroma)
    let c1 = (a1 * a1 + b1 * b1).sqrt();
    let c2 = (a2 * a2 + b2 * b2).sqrt();
    let c_avg = (c1 + c2) / 2.0;

    // Calculate G factor
    let c_avg_pow7 = c_avg.powi(7);
    let g = 0.5 * (1.0 - (c_avg_pow7 / (c_avg_pow7 + 6103515625.0_f32)).sqrt()); // 25^7 = 6103515625

    // Calculate a' (adjusted a)
    let a1_prime = a1 * (1.0 + g);
    let a2_prime = a2 * (1.0 + g);

    // Calculate C' (adjusted chroma)
    let c1_prime = (a1_prime * a1_prime + b1 * b1).sqrt();
    let c2_prime = (a2_prime * a2_prime + b2 * b2).sqrt();

    // Calculate h' (hue angle in radians)
    let h1_prime = if a1_prime == 0.0 && b1 == 0.0 {
        0.0
    } else {
        let h = b1.atan2(a1_prime);
        if h < 0.0 {
            h + 2.0 * PI
        } else {
            h
        }
    };

    let h2_prime = if a2_prime == 0.0 && b2 == 0.0 {
        0.0
    } else {
        let h = b2.atan2(a2_prime);
        if h < 0.0 {
            h + 2.0 * PI
        } else {
            h
        }
    };

    // Calculate differences
    let delta_l = l2 - l1;
    let delta_c = c2_prime - c1_prime;

    // Calculate delta h'
    let delta_h_prime = if c1_prime * c2_prime == 0.0 {
        0.0
    } else {
        let dh = h2_prime - h1_prime;
        if dh.abs() <= PI {
            dh
        } else if dh > PI {
            dh - 2.0 * PI
        } else {
            dh + 2.0 * PI
        }
    };

    // Calculate delta H'
    let delta_h = 2.0 * (c1_prime * c2_prime).sqrt() * (delta_h_prime / 2.0).sin();

    // Calculate average values
    let l_avg = (l1 + l2) / 2.0;
    let c_avg_prime = (c1_prime + c2_prime) / 2.0;

    // Calculate h' average
    let h_avg_prime = if c1_prime * c2_prime == 0.0 {
        h1_prime + h2_prime
    } else {
        let sum = h1_prime + h2_prime;
        let diff = (h1_prime - h2_prime).abs();
        if diff <= PI {
            sum / 2.0
        } else if sum < 2.0 * PI {
            (sum + 2.0 * PI) / 2.0
        } else {
            (sum - 2.0 * PI) / 2.0
        }
    };

    // Calculate T
    let t = 1.0 - 0.17 * (h_avg_prime - PI / 6.0).cos()
        + 0.24 * (2.0 * h_avg_prime).cos()
        + 0.32 * (3.0 * h_avg_prime + PI / 30.0).cos()
        - 0.20 * (4.0 * h_avg_prime - 63.0 * PI / 180.0).cos();

    // Calculate S_L, S_C, S_H
    let l_avg_minus_50_sq = (l_avg - 50.0).powi(2);
    let s_l = 1.0 + (0.015 * l_avg_minus_50_sq) / (20.0 + l_avg_minus_50_sq).sqrt();
    let s_c = 1.0 + 0.045 * c_avg_prime;
    let s_h = 1.0 + 0.015 * c_avg_prime * t;

    // Calculate R_T (rotation term)
    let delta_theta = 30.0 * (-(((h_avg_prime * 180.0 / PI) - 275.0) / 25.0).powi(2)).exp();
    let r_c = 2.0 * (c_avg_prime.powi(7) / (c_avg_prime.powi(7) + 6103515625.0_f32)).sqrt();
    let r_t = -(r_c * (2.0 * delta_theta * PI / 180.0).sin());

    // Calculate final delta E 2000
    let term1 = delta_l / (k_l * s_l);
    let term2 = delta_c / (k_c * s_c);
    let term3 = delta_h / (k_h * s_h);
    let term4 = r_t * (delta_c / (k_c * s_c)) * (delta_h / (k_h * s_h));

    (term1 * term1 + term2 * term2 + term3 * term3 + term4).sqrt()
}

/// Calculate color similarity score between two hex colors
/// Returns a score from 0.0 (opposite) to 1.0 (identical)
/// Uses Delta-E 2000 for perceptually accurate comparison
pub fn color_similarity(hex1: &str, hex2: &str) -> f32 {
    match (hex_to_lab(hex1), hex_to_lab(hex2)) {
        (Some(lab1), Some(lab2)) => {
            let distance = delta_e_2000(&lab1, &lab2);
            // Convert distance to similarity (0-1 range)
            // Delta-E 2000 values: 0 = identical, 1 = barely noticeable, 100 = very different
            // Use a curve that's more sensitive to small differences
            (1.0 - (distance / 100.0).powf(0.7)).max(0.0)
        }
        _ => 0.0,
    }
}

/// Find the best color match between two palettes, weighted by color dominance
/// Each color's contribution is scaled by its weight (proportion of the image)
/// Returns a weighted similarity score (0.0-1.0)
pub fn palette_similarity_weighted(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    // Normalize weights in case they don't sum to 1
    let sum1: f32 = weights1.iter().sum();
    let sum2: f32 = weights2.iter().sum();
    let norm_weights1: Vec<f32> = if sum1 > 0.0 {
        weights1.iter().map(|w| w / sum1).collect()
    } else {
        vec![1.0 / colors1.len() as f32; colors1.len()]
    };
    let norm_weights2: Vec<f32> = if sum2 > 0.0 {
        weights2.iter().map(|w| w / sum2).collect()
    } else {
        vec![1.0 / colors2.len() as f32; colors2.len()]
    };

    let mut total_similarity = 0.0;

    // For each color in palette 1, find best match in palette 2
    // Weight the match by both the source color's weight and the best match's weight
    for (i, c1) in colors1.iter().enumerate() {
        let w1 = norm_weights1.get(i).copied().unwrap_or(0.0);
        if w1 < 0.01 {
            continue; // Skip very minor colors
        }

        let mut best_sim = 0.0;

        for (j, c2) in colors2.iter().enumerate() {
            let w2 = norm_weights2.get(j).copied().unwrap_or(0.0);
            let sim = color_similarity(c1, c2);

            // Boost similarity when matching dominant colors with dominant colors
            let weight_boost = (w2 * 2.0).min(1.0);
            let boosted_sim = sim * (0.7 + 0.3 * weight_boost);

            if boosted_sim > best_sim {
                best_sim = boosted_sim;
            }
        }

        // Scale contribution by source color's weight
        total_similarity += best_sim * w1;
    }

    total_similarity
}

/// Calculate brightness of a hex color (0.0-1.0)
pub fn color_brightness(hex: &str) -> f32 {
    match hex_to_rgb(hex) {
        Some((r, g, b)) => {
            // Perceived brightness formula
            (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0
        }
        None => 0.5,
    }
}

/// Calculate saturation of a hex color (0.0-1.0)
pub fn color_saturation(hex: &str) -> f32 {
    match hex_to_rgb(hex) {
        Some((r, g, b)) => {
            let r = r as f32 / 255.0;
            let g = g as f32 / 255.0;
            let b = b as f32 / 255.0;
            let max = r.max(g).max(b);
            let min = r.min(g).min(b);
            if max == 0.0 {
                0.0
            } else {
                (max - min) / max
            }
        }
        None => 0.0,
    }
}

/// Calculate overall image similarity based on color profile
/// Returns a score from 0.0 (very different) to 1.0 (very similar)
pub fn image_similarity(colors1: &[String], colors2: &[String]) -> f32 {
    // Use equal weights for backward compatibility
    let weights1: Vec<f32> = vec![1.0 / colors1.len().max(1) as f32; colors1.len()];
    let weights2: Vec<f32> = vec![1.0 / colors2.len().max(1) as f32; colors2.len()];
    image_similarity_weighted(colors1, &weights1, colors2, &weights2)
}

/// Calculate overall image similarity based on color profile with weights
/// Returns a score from 0.0 (very different) to 1.0 (very similar)
pub fn image_similarity_weighted(
    colors1: &[String],
    weights1: &[f32],
    colors2: &[String],
    weights2: &[f32],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    // Component 1: Palette similarity (color matching) with weights
    let color_sim = palette_similarity_weighted(colors1, weights1, colors2, weights2);

    // Component 2: Weighted brightness similarity
    let sum1: f32 = weights1.iter().sum();
    let sum2: f32 = weights2.iter().sum();
    let bright1: f32 = colors1
        .iter()
        .zip(weights1.iter())
        .map(|(c, w)| color_brightness(c) * w)
        .sum::<f32>()
        / sum1.max(0.001);
    let bright2: f32 = colors2
        .iter()
        .zip(weights2.iter())
        .map(|(c, w)| color_brightness(c) * w)
        .sum::<f32>()
        / sum2.max(0.001);
    let bright_sim = 1.0 - (bright1 - bright2).abs();

    // Component 3: Weighted saturation similarity
    let sat1: f32 = colors1
        .iter()
        .zip(weights1.iter())
        .map(|(c, w)| color_saturation(c) * w)
        .sum::<f32>()
        / sum1.max(0.001);
    let sat2: f32 = colors2
        .iter()
        .zip(weights2.iter())
        .map(|(c, w)| color_saturation(c) * w)
        .sum::<f32>()
        / sum2.max(0.001);
    let sat_sim = 1.0 - (sat1 - sat2).abs();

    // Weighted combination
    color_sim * 0.6 + bright_sim * 0.25 + sat_sim * 0.15
}

/// Find similar wallpapers based on color profile
/// Returns Vec of (similarity_score, wallpaper_index) sorted by similarity
pub fn find_similar_wallpapers(
    target_colors: &[String],
    all_wallpapers: &[(usize, &[String])], // (index, colors)
    limit: usize,
) -> Vec<(f32, usize)> {
    let mut similarities: Vec<(f32, usize)> = all_wallpapers
        .iter()
        .map(|(idx, colors)| {
            let sim = image_similarity(target_colors, colors);
            (sim, *idx)
        })
        .collect();

    // Sort by similarity descending
    similarities.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));

    similarities.into_iter().take(limit).collect()
}

/// Check if a path is a supported image file
pub fn is_image_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            let ext = e.to_lowercase();
            IMAGE_EXTENSIONS.iter().any(|&supported| supported == ext)
        })
        .unwrap_or(false)
}

/// Expand tilde (~) in path
pub fn expand_tilde(path: &str) -> std::path::PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- hex_to_rgb ---

    #[test]
    fn test_hex_to_rgb_with_hash() {
        assert_eq!(hex_to_rgb("#FF0000"), Some((255, 0, 0)));
        assert_eq!(hex_to_rgb("#00FF00"), Some((0, 255, 0)));
        assert_eq!(hex_to_rgb("#0000FF"), Some((0, 0, 255)));
        assert_eq!(hex_to_rgb("#000000"), Some((0, 0, 0)));
        assert_eq!(hex_to_rgb("#FFFFFF"), Some((255, 255, 255)));
    }

    #[test]
    fn test_hex_to_rgb_without_hash() {
        assert_eq!(hex_to_rgb("FF0000"), Some((255, 0, 0)));
        assert_eq!(hex_to_rgb("00ff00"), Some((0, 255, 0)));
    }

    #[test]
    fn test_hex_to_rgb_lowercase() {
        assert_eq!(hex_to_rgb("#ff8040"), Some((255, 128, 64)));
    }

    #[test]
    fn test_hex_to_rgb_invalid() {
        assert_eq!(hex_to_rgb("#FFF"), None); // too short
        assert_eq!(hex_to_rgb("#GGGGGG"), None); // invalid hex chars
        assert_eq!(hex_to_rgb(""), None); // empty
        assert_eq!(hex_to_rgb("#FF00FF00"), None); // too long
    }

    // --- hex_to_lab ---

    #[test]
    fn test_hex_to_lab_white() {
        let lab = hex_to_lab("#FFFFFF").unwrap();
        assert!((lab.l - 100.0).abs() < 1.0, "White L should be ~100, got {}", lab.l);
        assert!(lab.a.abs() < 1.0, "White a should be ~0, got {}", lab.a);
        assert!(lab.b.abs() < 1.0, "White b should be ~0, got {}", lab.b);
    }

    #[test]
    fn test_hex_to_lab_black() {
        let lab = hex_to_lab("#000000").unwrap();
        assert!(lab.l.abs() < 1.0, "Black L should be ~0, got {}", lab.l);
    }

    #[test]
    fn test_hex_to_lab_invalid() {
        assert!(hex_to_lab("#GGG").is_none());
    }

    // --- hex_to_hsl ---

    #[test]
    fn test_hex_to_hsl_red() {
        let (h, s, l) = hex_to_hsl("#FF0000").unwrap();
        assert!(h.abs() < 1.0 || (h - 360.0).abs() < 1.0, "Red hue should be ~0, got {}", h);
        assert!((s - 1.0).abs() < 0.01, "Red saturation should be 1.0, got {}", s);
        assert!((l - 0.5).abs() < 0.01, "Red lightness should be 0.5, got {}", l);
    }

    #[test]
    fn test_hex_to_hsl_gray() {
        let (h, s, l) = hex_to_hsl("#808080").unwrap();
        assert!((h - 0.0).abs() < 0.01, "Gray hue should be 0 (achromatic), got {}", h);
        assert!((s - 0.0).abs() < 0.01, "Gray saturation should be 0, got {}", s);
        assert!((l - 0.502).abs() < 0.02, "Gray lightness should be ~0.5, got {}", l);
    }

    #[test]
    fn test_hex_to_hsl_blue() {
        let (h, _s, _l) = hex_to_hsl("#0000FF").unwrap();
        assert!((h - 240.0).abs() < 1.0, "Blue hue should be ~240, got {}", h);
    }

    #[test]
    fn test_hex_to_hsl_invalid() {
        assert!(hex_to_hsl("invalid").is_none());
    }

    // --- delta_e_2000 ---

    #[test]
    fn test_delta_e_2000_identical() {
        let lab = hex_to_lab("#FF0000").unwrap();
        let d = delta_e_2000(&lab, &lab);
        assert!(d.abs() < 0.001, "Identical colors should have delta_e ~0, got {}", d);
    }

    #[test]
    fn test_delta_e_2000_black_white() {
        let black = hex_to_lab("#000000").unwrap();
        let white = hex_to_lab("#FFFFFF").unwrap();
        let d = delta_e_2000(&black, &white);
        assert!(d > 50.0, "Black vs white should have large delta_e, got {}", d);
    }

    #[test]
    fn test_delta_e_2000_similar_colors() {
        let c1 = hex_to_lab("#FF0000").unwrap();
        let c2 = hex_to_lab("#FE0000").unwrap();
        let d = delta_e_2000(&c1, &c2);
        assert!(d < 2.0, "Very similar reds should have small delta_e, got {}", d);
    }

    #[test]
    fn test_delta_e_2000_symmetry() {
        let c1 = hex_to_lab("#FF0000").unwrap();
        let c2 = hex_to_lab("#00FF00").unwrap();
        let d1 = delta_e_2000(&c1, &c2);
        let d2 = delta_e_2000(&c2, &c1);
        assert!((d1 - d2).abs() < 0.001, "delta_e should be symmetric: {} vs {}", d1, d2);
    }

    // --- detect_harmony ---

    #[test]
    fn test_detect_harmony_analogous() {
        // Red and orange-red should be analogous (close hues)
        let colors1 = vec!["#FF0000".into()];
        let colors2 = vec!["#FF3300".into()];
        let weights = vec![1.0];
        let (harmony, strength) = detect_harmony(&colors1, &weights, &colors2, &weights);
        assert_eq!(harmony, ColorHarmony::Analogous);
        assert!(strength > 0.0, "Strength should be positive, got {}", strength);
    }

    #[test]
    fn test_detect_harmony_complementary() {
        // Red and cyan should be complementary (180 degrees apart)
        let colors1 = vec!["#FF0000".into()];
        let colors2 = vec!["#00FFFF".into()];
        let weights = vec![1.0];
        let (harmony, _) = detect_harmony(&colors1, &weights, &colors2, &weights);
        assert_eq!(harmony, ColorHarmony::Complementary);
    }

    #[test]
    fn test_detect_harmony_empty() {
        let empty: Vec<String> = vec![];
        let colors = vec!["#FF0000".into()];
        let weights = vec![1.0];
        let (harmony, strength) = detect_harmony(&empty, &[], &colors, &weights);
        assert_eq!(harmony, ColorHarmony::None);
        assert!((strength - 0.0).abs() < 0.001);
    }

    // --- color_similarity ---

    #[test]
    fn test_color_similarity_identical() {
        let sim = color_similarity("#FF0000", "#FF0000");
        assert!((sim - 1.0).abs() < 0.01, "Identical colors should have similarity ~1.0, got {}", sim);
    }

    #[test]
    fn test_color_similarity_very_different() {
        let sim = color_similarity("#000000", "#FFFFFF");
        assert!(sim < 0.5, "Black vs white should have low similarity, got {}", sim);
    }

    #[test]
    fn test_color_similarity_invalid() {
        let sim = color_similarity("invalid", "#FF0000");
        assert!((sim - 0.0).abs() < 0.001, "Invalid color should return 0.0");
    }

    // --- palette_similarity_weighted ---

    #[test]
    fn test_palette_similarity_weighted_same() {
        let colors = vec!["#FF0000".into(), "#00FF00".into()];
        let weights = vec![0.5, 0.5];
        let sim = palette_similarity_weighted(&colors, &weights, &colors, &weights);
        assert!(sim > 0.9, "Same palette should have high similarity, got {}", sim);
    }

    #[test]
    fn test_palette_similarity_weighted_empty() {
        let empty: Vec<String> = vec![];
        let colors = vec!["#FF0000".into()];
        assert_eq!(palette_similarity_weighted(&empty, &[], &colors, &[1.0]), 0.0);
        assert_eq!(palette_similarity_weighted(&colors, &[1.0], &empty, &[]), 0.0);
    }

    // --- color_brightness ---

    #[test]
    fn test_color_brightness_white() {
        let b = color_brightness("#FFFFFF");
        assert!((b - 1.0).abs() < 0.01, "White brightness should be ~1.0, got {}", b);
    }

    #[test]
    fn test_color_brightness_black() {
        let b = color_brightness("#000000");
        assert!(b.abs() < 0.01, "Black brightness should be ~0.0, got {}", b);
    }

    #[test]
    fn test_color_brightness_invalid() {
        let b = color_brightness("invalid");
        assert!((b - 0.5).abs() < 0.01, "Invalid color brightness should default to 0.5");
    }

    // --- color_saturation ---

    #[test]
    fn test_color_saturation_pure_red() {
        let s = color_saturation("#FF0000");
        assert!((s - 1.0).abs() < 0.01, "Pure red saturation should be 1.0, got {}", s);
    }

    #[test]
    fn test_color_saturation_gray() {
        let s = color_saturation("#808080");
        assert!(s.abs() < 0.01, "Gray saturation should be ~0.0, got {}", s);
    }

    #[test]
    fn test_color_saturation_white() {
        let s = color_saturation("#FFFFFF");
        assert!(s.abs() < 0.01, "White saturation should be 0.0, got {}", s);
    }

    // --- is_image_file ---

    #[test]
    fn test_is_image_file_supported() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.jpeg")));
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("photo.webp")));
        assert!(is_image_file(Path::new("photo.bmp")));
        assert!(is_image_file(Path::new("photo.gif")));
    }

    #[test]
    fn test_is_image_file_case_insensitive() {
        assert!(is_image_file(Path::new("photo.JPG")));
        assert!(is_image_file(Path::new("photo.PNG")));
        assert!(is_image_file(Path::new("photo.WebP")));
    }

    #[test]
    fn test_is_image_file_unsupported() {
        assert!(!is_image_file(Path::new("document.txt")));
        assert!(!is_image_file(Path::new("video.mp4")));
        assert!(!is_image_file(Path::new("noextension")));
        assert!(!is_image_file(Path::new(".hidden")));
    }

    // --- expand_tilde ---

    #[test]
    fn test_expand_tilde_with_tilde() {
        let expanded = expand_tilde("~/documents/test.png");
        let expanded_str = expanded.to_string_lossy();
        assert!(!expanded_str.starts_with("~/"), "Should expand ~, got {}", expanded_str);
        assert!(expanded_str.ends_with("documents/test.png"));
    }

    #[test]
    fn test_expand_tilde_absolute_path() {
        let path = "/absolute/path/file.png";
        let expanded = expand_tilde(path);
        assert_eq!(expanded.to_string_lossy(), path, "Absolute path should be unchanged");
    }

    // --- image_similarity ---

    #[test]
    fn test_image_similarity_identical() {
        let colors = vec!["#FF0000".into(), "#00FF00".into(), "#0000FF".into()];
        let sim = image_similarity(&colors, &colors);
        assert!(sim > 0.9, "Identical palettes should have high image similarity, got {}", sim);
    }

    #[test]
    fn test_image_similarity_empty() {
        let empty: Vec<String> = vec![];
        let colors = vec!["#FF0000".into()];
        assert_eq!(image_similarity(&empty, &colors), 0.0);
    }

    // --- ColorHarmony ---

    #[test]
    fn test_color_harmony_bonus() {
        assert!(ColorHarmony::Analogous.bonus() > ColorHarmony::None.bonus());
        assert!(ColorHarmony::Complementary.bonus() > ColorHarmony::None.bonus());
        assert_eq!(ColorHarmony::None.bonus(), 0.0);
    }

    #[test]
    fn test_color_harmony_name() {
        assert_eq!(ColorHarmony::Analogous.name(), "Analogous");
        assert_eq!(ColorHarmony::Complementary.name(), "Complementary");
        assert_eq!(ColorHarmony::Triadic.name(), "Triadic");
        assert_eq!(ColorHarmony::SplitComplementary.name(), "Split-Complementary");
        assert_eq!(ColorHarmony::None.name(), "None");
    }

    // --- find_similar_wallpapers ---

    #[test]
    fn test_find_similar_wallpapers_returns_sorted() {
        let target = vec!["#FF0000".into()];
        let c0 = [String::from("#0000FF")];
        let c1 = [String::from("#FF0000")];
        let c2 = [String::from("#FF1100")];
        let candidates: Vec<(usize, &[String])> = vec![
            (0, &c0),
            (1, &c1),
            (2, &c2),
        ];
        let results = find_similar_wallpapers(&target, &candidates, 3);
        assert!(!results.is_empty());
        // Most similar (index 1, identical) should be first
        assert_eq!(results[0].1, 1, "Identical color should be best match");
        // Scores should be descending
        for w in results.windows(2) {
            assert!(w[0].0 >= w[1].0, "Results should be sorted by similarity descending");
        }
    }
}
