use palette::{IntoColor, Lab, Srgb};
use std::collections::HashMap;
use std::sync::{OnceLock, RwLock};

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
    #[cfg(test)]
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

/// Minimum delta to consider a color chromatic (not gray)
const HSL_ACHROMATIC_THRESHOLD: f32 = 0.0001;

/// Minimum saturation for a color to be considered when detecting harmony
const COLOR_MIN_SATURATION: f32 = 0.15;

/// Lightness range for saturated colors used in harmony detection
const COLOR_MIN_LIGHTNESS: f32 = 0.1;
const COLOR_MAX_LIGHTNESS: f32 = 0.9;

/// Hue angle thresholds for harmony classification (degrees)
const HARMONY_ANALOGOUS_MAX: f32 = 30.0;
const HARMONY_COMPLEMENTARY_MIN: f32 = 165.0;
const HARMONY_COMPLEMENTARY_MAX: f32 = 195.0;
const HARMONY_TRIADIC_MIN: f32 = 105.0;
const HARMONY_TRIADIC_MAX: f32 = 135.0;
const HARMONY_SPLIT_MIN: f32 = 135.0;
const HARMONY_SPLIT_MAX: f32 = 165.0;

#[derive(Clone)]
struct ColorFeatures {
    lab: Lab,
    hue: f32,
    saturation: f32,
    lightness: f32,
    brightness: f32,
}

static COLOR_FEATURE_CACHE: OnceLock<RwLock<HashMap<u32, ColorFeatures>>> = OnceLock::new();

fn color_feature_cache() -> &'static RwLock<HashMap<u32, ColorFeatures>> {
    COLOR_FEATURE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn parse_hex_u24(hex: &str) -> Option<u32> {
    let hex = hex.trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

fn rgb_from_u24(value: u32) -> (u8, u8, u8) {
    let r = ((value >> 16) & 0xff) as u8;
    let g = ((value >> 8) & 0xff) as u8;
    let b = (value & 0xff) as u8;
    (r, g, b)
}

fn compute_hsl_from_rgb(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let lightness = (max + min) / 2.0;

    if delta < HSL_ACHROMATIC_THRESHOLD {
        return (0.0, 0.0, lightness);
    }

    let saturation = if lightness > 0.5 {
        delta / (2.0 - max - min)
    } else {
        delta / (max + min)
    };

    let hue = if (max - r).abs() < HSL_ACHROMATIC_THRESHOLD {
        60.0 * (((g - b) / delta) % 6.0)
    } else if (max - g).abs() < HSL_ACHROMATIC_THRESHOLD {
        60.0 * ((b - r) / delta + 2.0)
    } else {
        60.0 * ((r - g) / delta + 4.0)
    };

    let hue = if hue < 0.0 { hue + 360.0 } else { hue };
    (hue, saturation, lightness)
}

fn compute_color_features((r, g, b): (u8, u8, u8)) -> ColorFeatures {
    let rf = r as f32 / 255.0;
    let gf = g as f32 / 255.0;
    let bf = b as f32 / 255.0;
    let (hue, saturation, lightness) = compute_hsl_from_rgb(rf, gf, bf);
    let rgb = Srgb::new(rf, gf, bf);

    ColorFeatures {
        lab: rgb.into_color(),
        hue,
        saturation,
        lightness,
        brightness: (0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32) / 255.0,
    }
}

fn color_features(hex: &str) -> Option<ColorFeatures> {
    let key = parse_hex_u24(hex)?;

    if let Ok(cache) = color_feature_cache().read() {
        if let Some(features) = cache.get(&key) {
            return Some(features.clone());
        }
    }

    let features = compute_color_features(rgb_from_u24(key));

    if let Ok(mut cache) = color_feature_cache().write() {
        cache.entry(key).or_insert_with(|| features.clone());
    }

    Some(features)
}

/// Parse hex color string to RGB tuple
/// Supports "#RRGGBB" and "RRGGBB" formats
pub fn hex_to_rgb(hex: &str) -> Option<(u8, u8, u8)> {
    parse_hex_u24(hex).map(rgb_from_u24)
}

/// Convert hex color to LAB color space
pub fn hex_to_lab(hex: &str) -> Option<Lab> {
    color_features(hex).map(|f| f.lab)
}

/// Convert hex color to HSL and return hue (0-360), saturation (0-1), lightness (0-1)
pub fn hex_to_hsl(hex: &str) -> Option<(f32, f32, f32)> {
    color_features(hex).map(|f| (f.hue, f.saturation, f.lightness))
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
                    if s > COLOR_MIN_SATURATION
                        && l > COLOR_MIN_LIGHTNESS
                        && l < COLOR_MAX_LIGHTNESS
                    {
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
            let (harmony, strength) = if diff < HARMONY_ANALOGOUS_MAX {
                // Analogous: similar hues
                (ColorHarmony::Analogous, 1.0 - diff / HARMONY_ANALOGOUS_MAX)
            } else if (HARMONY_COMPLEMENTARY_MIN..=HARMONY_COMPLEMENTARY_MAX).contains(&diff) {
                // Complementary: opposite hues
                let center_diff = (diff - 180.0).abs();
                (ColorHarmony::Complementary, 1.0 - center_diff / 15.0)
            } else if (HARMONY_TRIADIC_MIN..=HARMONY_TRIADIC_MAX).contains(&diff) {
                // Triadic: 120° apart
                let center_diff = (diff - 120.0).abs();
                (ColorHarmony::Triadic, 1.0 - center_diff / 15.0)
            } else if (HARMONY_SPLIT_MIN..HARMONY_SPLIT_MAX).contains(&diff) {
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

/// Calculate Delta E 2000 (CIEDE2000) - perceptually uniform color difference
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
    let g = 0.5 * (1.0 - (c_avg_pow7 / (c_avg_pow7 + 6103515625.0_f32)).sqrt()); // 25^7

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
            (1.0 - (distance / 100.0).powf(0.7)).max(0.0)
        }
        _ => 0.0,
    }
}

/// Calculate brightness of a hex color (0.0-1.0)
pub fn color_brightness(hex: &str) -> f32 {
    color_features(hex).map(|f| f.brightness).unwrap_or(0.5)
}

/// Calculate saturation of a hex color (0.0-1.0)
pub fn color_saturation(hex: &str) -> f32 {
    color_features(hex).map(|f| f.saturation).unwrap_or(0.0)
}
