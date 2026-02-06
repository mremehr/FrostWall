use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::process::Command;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Screen {
    pub name: String,
    pub width: u32,
    pub height: u32,
    pub orientation: Orientation,
    pub aspect_category: AspectCategory,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Orientation {
    Landscape,
    Portrait,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AspectCategory {
    Ultrawide, // 21:9, 32:9, etc.
    Landscape, // 16:9, 16:10
    Portrait,  // 9:16, 10:16
    Square,    // ~1:1
}

impl Screen {
    pub fn new(name: String, width: u32, height: u32) -> Self {
        let (orientation, aspect_category) = Self::analyze_aspect(width, height);
        Self {
            name,
            width,
            height,
            orientation,
            aspect_category,
        }
    }

    fn analyze_aspect(width: u32, height: u32) -> (Orientation, AspectCategory) {
        let ratio = width as f32 / height as f32;

        let orientation = if ratio >= 1.0 {
            Orientation::Landscape
        } else {
            Orientation::Portrait
        };

        // Use the larger/smaller dimension for consistent ratio calculation
        let normalized_ratio = if ratio >= 1.0 { ratio } else { 1.0 / ratio };

        let category = if normalized_ratio >= 2.0 {
            // 32:9 = 3.56, 21:9 = 2.33
            AspectCategory::Ultrawide
        } else if normalized_ratio >= 1.2 {
            // 16:9 = 1.78, 16:10 = 1.6
            if orientation == Orientation::Landscape {
                AspectCategory::Landscape
            } else {
                AspectCategory::Portrait
            }
        } else {
            // Close to 1:1
            AspectCategory::Square
        };

        (orientation, category)
    }
}

/// Detect connected screens using niri msg outputs
pub async fn detect_screens() -> Result<Vec<Screen>> {
    // Try niri first
    if let Ok(screens) = detect_niri().await {
        return Ok(screens);
    }

    // Fallback to wlr-randr
    if let Ok(screens) = detect_wlr_randr().await {
        return Ok(screens);
    }

    anyhow::bail!("Could not detect screens. Make sure niri or wlr-randr is available.")
}

async fn detect_niri() -> Result<Vec<Screen>> {
    let output = Command::new("niri")
        .args(["msg", "outputs"])
        .output()
        .context("Failed to run niri msg outputs")?;

    if !output.status.success() {
        anyhow::bail!("niri msg outputs failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_niri_output(&stdout)
}

fn parse_niri_output(output: &str) -> Result<Vec<Screen>> {
    let mut screens = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_width: Option<u32> = None;
    let mut current_height: Option<u32> = None;

    for line in output.lines() {
        let trimmed = line.trim();

        // Output name line: "Output "Description" (DP-2)"
        if line.starts_with("Output ") {
            // Save previous screen if complete
            if let (Some(name), Some(w), Some(h)) = (&current_name, current_width, current_height) {
                screens.push(Screen::new(name.clone(), w, h));
            }

            // Extract output name from parentheses: (DP-2)
            current_name = line
                .rsplit('(')
                .next()
                .and_then(|s| s.strip_suffix(')'))
                .map(String::from);
            current_width = None;
            current_height = None;
        }

        // Logical size line: "Logical size: 1080x1920" (already includes rotation!)
        if trimmed.starts_with("Logical size:") {
            if let Some(size_part) = trimmed.split_whitespace().nth(2) {
                let parts: Vec<&str> = size_part.split('x').collect();
                if parts.len() == 2 {
                    current_width = parts[0].parse().ok();
                    current_height = parts[1].parse().ok();
                }
            }
        }
    }

    // Don't forget the last screen
    if let (Some(name), Some(w), Some(h)) = (current_name, current_width, current_height) {
        screens.push(Screen::new(name, w, h));
    }

    if screens.is_empty() {
        anyhow::bail!("No screens found in niri output");
    }

    Ok(screens)
}

async fn detect_wlr_randr() -> Result<Vec<Screen>> {
    let output = Command::new("wlr-randr")
        .output()
        .context("Failed to run wlr-randr")?;

    if !output.status.success() {
        anyhow::bail!("wlr-randr failed");
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    parse_wlr_randr_output(&stdout)
}

fn parse_wlr_randr_output(output: &str) -> Result<Vec<Screen>> {
    let mut screens = Vec::new();
    let mut current_name: Option<String> = None;
    let mut current_width: Option<u32> = None;
    let mut current_height: Option<u32> = None;
    let mut current_transform: Option<String> = None;

    for line in output.lines() {
        // Output name: "DP-1 "Samsung Electric..." (or similar)
        if !line.starts_with(' ') && !line.is_empty() {
            // Save previous screen if complete
            if let (Some(name), Some(w), Some(h)) = (&current_name, current_width, current_height) {
                let (final_w, final_h) = apply_transform(w, h, current_transform.as_deref());
                screens.push(Screen::new(name.clone(), final_w, final_h));
            }

            current_name = line.split_whitespace().next().map(String::from);
            current_width = None;
            current_height = None;
            current_transform = None;
        }

        // Resolution line with "current"
        if line.contains("current") && line.contains("px") {
            // Parse "  1920x1080 px, 144.000000 Hz (current)"
            let trimmed = line.trim();
            if let Some(res) = trimmed.split_whitespace().next() {
                let parts: Vec<&str> = res.split('x').collect();
                if parts.len() == 2 {
                    current_width = parts[0].parse().ok();
                    current_height = parts[1].parse().ok();
                }
            }
        }

        // Transform line: "  Transform: 90" or "  Transform: normal"
        if line.trim().starts_with("Transform:") {
            current_transform = line.split(':').nth(1).map(|s| s.trim().to_string());
        }
    }

    // Don't forget the last screen
    if let (Some(name), Some(w), Some(h)) = (current_name, current_width, current_height) {
        let (final_w, final_h) = apply_transform(w, h, current_transform.as_deref());
        screens.push(Screen::new(name, final_w, final_h));
    }

    if screens.is_empty() {
        anyhow::bail!("No screens found in wlr-randr output");
    }

    Ok(screens)
}

/// Apply transform rotation - swap dimensions for 90/270 degree rotations
fn apply_transform(width: u32, height: u32, transform: Option<&str>) -> (u32, u32) {
    match transform {
        Some("90") | Some("270") | Some("flipped-90") | Some("flipped-270") => (height, width),
        _ => (width, height),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_aspect_categories() {
        // Ultrawide 21:9
        let screen = Screen::new("test".into(), 2560, 1080);
        assert_eq!(screen.aspect_category, AspectCategory::Ultrawide);

        // Standard 16:9
        let screen = Screen::new("test".into(), 1920, 1080);
        assert_eq!(screen.aspect_category, AspectCategory::Landscape);

        // Portrait (rotated 16:9)
        let screen = Screen::new("test".into(), 1080, 1920);
        assert_eq!(screen.aspect_category, AspectCategory::Portrait);

        // Super ultrawide 32:9
        let screen = Screen::new("test".into(), 5120, 1440);
        assert_eq!(screen.aspect_category, AspectCategory::Ultrawide);
    }
}
