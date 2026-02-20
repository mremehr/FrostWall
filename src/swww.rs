use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;

#[derive(Debug, Clone)]
pub struct Transition {
    pub transition_type: TransitionType,
    pub duration: f32,
    pub fps: u32,
}

#[derive(Debug, Clone, Copy)]
pub enum TransitionType {
    Fade,
    Wipe,
    Grow,
    Center,
    Outer,
    None,
}

/// How to resize/fit the wallpaper to the screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum ResizeMode {
    /// Resize to fill the screen, cropping parts that don't fit (default)
    #[default]
    Crop,
    /// Resize to fit inside the screen, preserving aspect ratio (adds padding)
    Fit,
    /// Don't resize, center the image (adds padding if smaller)
    No,
    /// Stretch to fill (distorts aspect ratio)
    Stretch,
}

impl ResizeMode {
    fn as_str(&self) -> &'static str {
        match self {
            ResizeMode::Crop => "crop",
            ResizeMode::Fit => "fit",
            ResizeMode::No => "no",
            ResizeMode::Stretch => "stretch",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ResizeMode::Crop => "Crop (fill)",
            ResizeMode::Fit => "Fit (letterbox)",
            ResizeMode::No => "Center (no resize)",
            ResizeMode::Stretch => "Stretch",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            ResizeMode::Crop => ResizeMode::Fit,
            ResizeMode::Fit => ResizeMode::No,
            ResizeMode::No => ResizeMode::Stretch,
            ResizeMode::Stretch => ResizeMode::Crop,
        }
    }
}

/// Fill color for padding when image doesn't fill screen
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FillColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Default for FillColor {
    fn default() -> Self {
        // Black
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }
}

impl FillColor {
    pub fn black() -> Self {
        Self {
            r: 0,
            g: 0,
            b: 0,
            a: 255,
        }
    }

    fn to_hex(&self) -> String {
        format!("{:02x}{:02x}{:02x}{:02x}", self.r, self.g, self.b, self.a)
    }
}

impl Default for Transition {
    fn default() -> Self {
        Self {
            transition_type: TransitionType::Fade,
            duration: 1.0,
            fps: 60,
        }
    }
}

impl TransitionType {
    fn as_str(&self) -> &'static str {
        match self {
            TransitionType::Fade => "fade",
            TransitionType::Wipe => "wipe",
            TransitionType::Grow => "grow",
            TransitionType::Center => "center",
            TransitionType::Outer => "outer",
            TransitionType::None => "none",
        }
    }
}

/// Initialize swww daemon if not running
pub fn ensure_daemon() -> Result<()> {
    // Check if daemon is running
    let status = Command::new("swww").arg("query").output();

    match status {
        Ok(output) if output.status.success() => Ok(()),
        _ => {
            // Start daemon
            Command::new("swww-daemon")
                .spawn()
                .context("Failed to start swww-daemon")?;

            // Give it a moment to initialize
            std::thread::sleep(std::time::Duration::from_millis(100));
            Ok(())
        }
    }
}

/// Set wallpaper on a specific output with resize options
pub fn set_wallpaper(output: &str, path: &Path, transition: &Transition) -> Result<()> {
    set_wallpaper_with_resize(
        output,
        path,
        transition,
        ResizeMode::Crop,
        &FillColor::black(),
    )
}

/// Set wallpaper on a specific output with full control over resize behavior
pub fn set_wallpaper_with_resize(
    output: &str,
    path: &Path,
    transition: &Transition,
    resize_mode: ResizeMode,
    fill_color: &FillColor,
) -> Result<()> {
    ensure_daemon()?;

    let mut cmd = Command::new("swww");
    cmd.arg("img")
        .arg("-o")
        .arg(output)
        .arg(path)
        .arg("--resize")
        .arg(resize_mode.as_str())
        .arg("--fill-color")
        .arg(fill_color.to_hex())
        .arg("--transition-type")
        .arg(transition.transition_type.as_str())
        .arg("--transition-duration")
        .arg(transition.duration.to_string())
        .arg("--transition-fps")
        .arg(transition.fps.to_string());

    let output = cmd.output().context("Failed to run swww")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("swww failed: {}", stderr);
    }

    Ok(())
}
