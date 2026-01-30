use std::path::Path;
use palette::{IntoColor, Lab, Srgb};

/// Supported image file extensions
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

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

/// Calculate Delta E (CIE76) color distance between two LAB colors
/// Lower values = more similar, 0 = identical
/// < 1.0: Not perceptible by human eye
/// 1-2: Perceptible through close observation
/// 2-10: Perceptible at a glance
/// 11-49: Colors are more similar than opposite
/// 100: Colors are exact opposite
pub fn delta_e(lab1: &Lab, lab2: &Lab) -> f32 {
    let dl = lab1.l - lab2.l;
    let da = lab1.a - lab2.a;
    let db = lab1.b - lab2.b;
    (dl * dl + da * da + db * db).sqrt()
}

/// Calculate color similarity score between two hex colors
/// Returns a score from 0.0 (opposite) to 1.0 (identical)
pub fn color_similarity(hex1: &str, hex2: &str) -> f32 {
    match (hex_to_lab(hex1), hex_to_lab(hex2)) {
        (Some(lab1), Some(lab2)) => {
            let distance = delta_e(&lab1, &lab2);
            // Convert distance to similarity (0-1 range)
            // Max perceptual distance is ~100, but most are <50
            (1.0 - (distance / 100.0)).max(0.0)
        }
        _ => 0.0,
    }
}

/// Find the best color match between two palettes using LAB distance
/// Returns the average similarity of the best matches
pub fn palette_similarity(colors1: &[String], colors2: &[String]) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    let mut total_similarity = 0.0;

    // For each color in palette 1, find best match in palette 2
    for c1 in colors1 {
        let best_match = colors2
            .iter()
            .map(|c2| color_similarity(c1, c2))
            .fold(0.0f32, |a, b| a.max(b));
        total_similarity += best_match;
    }

    total_similarity / colors1.len() as f32
}

/// Auto-tag definitions based on color characteristics
pub struct TagDefinition {
    pub name: &'static str,
    pub colors: &'static [&'static str],  // Reference colors
    pub brightness_range: (f32, f32),      // (min, max) 0.0-1.0
    pub saturation_range: (f32, f32),      // (min, max) 0.0-1.0
}

/// Predefined tag definitions
pub const AUTO_TAG_DEFINITIONS: &[TagDefinition] = &[
    TagDefinition {
        name: "nature",
        colors: &["#228b22", "#006400", "#90ee90", "#2e8b57", "#32cd32"],
        brightness_range: (0.2, 0.8),
        saturation_range: (0.3, 1.0),
    },
    TagDefinition {
        name: "ocean",
        colors: &["#0077be", "#00bfff", "#1e90ff", "#4169e1", "#000080"],
        brightness_range: (0.3, 0.9),
        saturation_range: (0.4, 1.0),
    },
    TagDefinition {
        name: "forest",
        colors: &["#228b22", "#013220", "#355e3b", "#2e8b57", "#006400"],
        brightness_range: (0.1, 0.5),
        saturation_range: (0.3, 0.8),
    },
    TagDefinition {
        name: "sunset",
        colors: &["#ff6347", "#ff7f50", "#ffa500", "#ff4500", "#ff8c00"],
        brightness_range: (0.4, 0.9),
        saturation_range: (0.5, 1.0),
    },
    TagDefinition {
        name: "dark",
        colors: &["#000000", "#1a1a2e", "#16213e", "#0f0f0f", "#2d2d2d"],
        brightness_range: (0.0, 0.3),
        saturation_range: (0.0, 0.5),
    },
    TagDefinition {
        name: "bright",
        colors: &["#ffffff", "#f0f0f0", "#fffacd", "#ffffe0", "#f5f5f5"],
        brightness_range: (0.75, 1.0),
        saturation_range: (0.0, 0.3),
    },
    TagDefinition {
        name: "cyberpunk",
        colors: &["#ff00ff", "#00ffff", "#ff1493", "#9400d3", "#7b68ee"],
        brightness_range: (0.2, 0.7),
        saturation_range: (0.7, 1.0),
    },
    TagDefinition {
        name: "minimal",
        colors: &["#ffffff", "#f5f5f5", "#e0e0e0", "#fafafa", "#d3d3d3"],
        brightness_range: (0.8, 1.0),
        saturation_range: (0.0, 0.15),
    },
    TagDefinition {
        name: "mountain",
        colors: &["#708090", "#778899", "#b0c4de", "#87ceeb", "#4682b4"],
        brightness_range: (0.3, 0.8),
        saturation_range: (0.1, 0.5),
    },
    TagDefinition {
        name: "space",
        colors: &["#000000", "#191970", "#0d0d0d", "#1a1a2e", "#2e0854"],
        brightness_range: (0.0, 0.25),
        saturation_range: (0.0, 0.6),
    },
    TagDefinition {
        name: "autumn",
        colors: &["#d2691e", "#8b4513", "#cd853f", "#daa520", "#b8860b"],
        brightness_range: (0.3, 0.7),
        saturation_range: (0.4, 0.9),
    },
    TagDefinition {
        name: "pastel",
        colors: &["#ffb6c1", "#dda0dd", "#b0e0e6", "#98fb98", "#fafad2"],
        brightness_range: (0.7, 0.95),
        saturation_range: (0.2, 0.5),
    },
];

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
pub fn image_similarity(
    colors1: &[String],
    colors2: &[String],
) -> f32 {
    if colors1.is_empty() || colors2.is_empty() {
        return 0.0;
    }

    // Component 1: Palette similarity (color matching)
    let color_sim = palette_similarity(colors1, colors2);

    // Component 2: Brightness similarity
    let bright1: f32 = colors1.iter().map(|c| color_brightness(c)).sum::<f32>() / colors1.len() as f32;
    let bright2: f32 = colors2.iter().map(|c| color_brightness(c)).sum::<f32>() / colors2.len() as f32;
    let bright_sim = 1.0 - (bright1 - bright2).abs();

    // Component 3: Saturation similarity
    let sat1: f32 = colors1.iter().map(|c| color_saturation(c)).sum::<f32>() / colors1.len() as f32;
    let sat2: f32 = colors2.iter().map(|c| color_saturation(c)).sum::<f32>() / colors2.len() as f32;
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

/// Auto-tag a wallpaper based on its color palette
/// Returns Vec<(tag_name, confidence)> sorted by confidence
pub fn auto_tag_from_colors(colors: &[String]) -> Vec<(String, f32)> {
    if colors.is_empty() {
        return Vec::new();
    }

    // Calculate average brightness and saturation
    let avg_brightness: f32 = colors.iter()
        .map(|c| color_brightness(c))
        .sum::<f32>() / colors.len() as f32;

    let avg_saturation: f32 = colors.iter()
        .map(|c| color_saturation(c))
        .sum::<f32>() / colors.len() as f32;

    let mut tags = Vec::new();

    for def in AUTO_TAG_DEFINITIONS {
        let mut score = 0.0f32;

        // Check brightness match (0-0.3 points)
        if avg_brightness >= def.brightness_range.0 && avg_brightness <= def.brightness_range.1 {
            score += 0.3;
        }

        // Check saturation match (0-0.2 points)
        if avg_saturation >= def.saturation_range.0 && avg_saturation <= def.saturation_range.1 {
            score += 0.2;
        }

        // Check color similarity (0-0.5 points)
        let def_colors: Vec<String> = def.colors.iter().map(|s| s.to_string()).collect();
        let color_match = palette_similarity(colors, &def_colors);
        score += color_match * 0.5;

        if score >= 0.35 {
            tags.push((def.name.to_string(), score));
        }
    }

    // Sort by confidence descending
    tags.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    tags
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
