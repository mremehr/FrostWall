use std::path::Path;

use super::{WalColorMap, WalColors, WalSpecial};

const FALLBACK_COLOR: &str = "#808080";
const BASE_COLORS: usize = 5;

/// Generate a 16-color terminal palette from dominant colors.
///
/// Takes up to 5 dominant colors and expands them to 16 terminal colors:
/// - color0: darkest (background)
/// - color1-6: main colors
/// - color7: light foreground
/// - color8-14: lighter variants
/// - color15: brightest (white)
pub fn generate_palette(dominant_colors: &[String], wallpaper_path: &Path) -> WalColors {
    let colors = normalized_base_colors(dominant_colors);
    let (darkest, lightest) = palette_extremes(&colors);
    let accent_cyan = blend(colors[1], colors[3]);

    let color_map = WalColorMap {
        color0: darken(darkest, 0.2),
        color1: colors[0].to_string(),
        color2: colors[1].to_string(),
        color3: colors[2].to_string(),
        color4: colors[3].to_string(),
        color5: colors[4].to_string(),
        color6: accent_cyan.clone(),
        color7: lighten(lightest, 0.1),
        color8: lighten(darkest, 0.3),
        color9: lighten(colors[0], 0.2),
        color10: lighten(colors[1], 0.2),
        color11: lighten(colors[2], 0.2),
        color12: lighten(colors[3], 0.2),
        color13: lighten(colors[4], 0.2),
        color14: lighten(&accent_cyan, 0.2),
        color15: lighten(lightest, 0.3),
    };

    WalColors {
        wallpaper: wallpaper_path.to_string_lossy().to_string(),
        alpha: "100".to_string(),
        special: WalSpecial {
            background: color_map.color0.clone(),
            foreground: color_map.color7.clone(),
            cursor: color_map.color7.clone(),
        },
        colors: color_map,
    }
}

fn normalized_base_colors(dominant_colors: &[String]) -> Vec<&str> {
    let mut colors: Vec<&str> = dominant_colors
        .iter()
        .map(String::as_str)
        .take(BASE_COLORS)
        .collect();
    colors.resize(BASE_COLORS, FALLBACK_COLOR);
    colors
}

fn palette_extremes<'a>(colors: &'a [&str]) -> (&'a str, &'a str) {
    let mut sorted_colors: Vec<(f32, &str)> =
        colors.iter().copied().map(|c| (luminance(c), c)).collect();
    sorted_colors.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));

    let darkest = sorted_colors
        .first()
        .map(|(_, color)| *color)
        .unwrap_or(FALLBACK_COLOR);
    let lightest = sorted_colors
        .last()
        .map(|(_, color)| *color)
        .unwrap_or(FALLBACK_COLOR);
    (darkest, lightest)
}

fn parse_hex(hex: &str) -> Option<(u8, u8, u8)> {
    crate::utils::hex_to_rgb(hex)
}

fn to_hex(r: u8, g: u8, b: u8) -> String {
    format!("#{:02x}{:02x}{:02x}", r, g, b)
}

fn luminance(hex: &str) -> f32 {
    if let Some((r, g, b)) = parse_hex(hex) {
        0.299 * (r as f32) + 0.587 * (g as f32) + 0.114 * (b as f32)
    } else {
        128.0
    }
}

fn lighten(hex: &str, amount: f32) -> String {
    if let Some((r, g, b)) = parse_hex(hex) {
        let r = (r as f32 + (255.0 - r as f32) * amount).min(255.0) as u8;
        let g = (g as f32 + (255.0 - g as f32) * amount).min(255.0) as u8;
        let b = (b as f32 + (255.0 - b as f32) * amount).min(255.0) as u8;
        to_hex(r, g, b)
    } else {
        hex.to_string()
    }
}

fn darken(hex: &str, amount: f32) -> String {
    if let Some((r, g, b)) = parse_hex(hex) {
        let r = (r as f32 * (1.0 - amount)).max(0.0) as u8;
        let g = (g as f32 * (1.0 - amount)).max(0.0) as u8;
        let b = (b as f32 * (1.0 - amount)).max(0.0) as u8;
        to_hex(r, g, b)
    } else {
        hex.to_string()
    }
}

fn blend(hex1: &str, hex2: &str) -> String {
    if let (Some((r1, g1, b1)), Some((r2, g2, b2))) = (parse_hex(hex1), parse_hex(hex2)) {
        let r = ((r1 as u16 + r2 as u16) / 2) as u8;
        let g = ((g1 as u16 + g2 as u16) / 2) as u8;
        let b = ((b1 as u16 + b2 as u16) / 2) as u8;
        to_hex(r, g, b)
    } else {
        hex1.to_string()
    }
}
