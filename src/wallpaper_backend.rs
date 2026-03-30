mod awww;

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::ffi::OsString;
use std::path::Path;
use std::sync::OnceLock;

trait WallpaperBackend {
    fn required_commands(&self) -> &'static [&'static str];
    fn set_wallpaper_with_resize(
        &self,
        output: &str,
        path: &Path,
        transition: &Transition,
        resize_mode: ResizeMode,
        fill_color: &FillColor,
    ) -> Result<()>;

    fn is_available(&self) -> bool {
        self.required_commands().iter().copied().all(command_exists)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum BackendKind {
    #[default]
    Auto,
    Awww,
}

impl BackendKind {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Awww => "awww",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct BackendConfig {
    #[serde(default)]
    pub kind: BackendKind,
}

#[derive(Debug, Clone)]
pub struct Transition {
    pub transition_type: TransitionType,
    pub duration: f32,
    pub fps: u32,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TransitionType {
    #[default]
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
    pub fn as_str(&self) -> &'static str {
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
        Self::black()
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

    pub fn to_hex(&self) -> String {
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
    pub fn as_str(&self) -> &'static str {
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

pub fn set_wallpaper_with_resize(
    backend_config: &BackendConfig,
    output: &str,
    path: &Path,
    transition: &Transition,
    resize_mode: ResizeMode,
    fill_color: &FillColor,
) -> Result<()> {
    backend_for(backend_config.kind)?.set_wallpaper_with_resize(
        output,
        path,
        transition,
        resize_mode,
        fill_color,
    )
}

fn backend_for(kind: BackendKind) -> Result<&'static dyn WallpaperBackend> {
    let backend = match resolve_backend_kind(kind)? {
        BackendKind::Awww => &awww::BACKEND as &dyn WallpaperBackend,
        BackendKind::Auto => unreachable!("auto must be resolved before dispatch"),
    };
    Ok(backend)
}

fn resolve_backend_kind(kind: BackendKind) -> Result<BackendKind> {
    match kind {
        BackendKind::Auto => resolve_auto_backend_kind(),
        BackendKind::Awww => {
            if awww::BACKEND.is_available() {
                Ok(BackendKind::Awww)
            } else {
                bail!(
                    "Configured wallpaper backend '{}' is not available.",
                    BackendKind::Awww.display_name()
                )
            }
        }
    }
}

fn resolve_auto_backend_kind() -> Result<BackendKind> {
    static AUTO_BACKEND: OnceLock<Result<BackendKind, String>> = OnceLock::new();

    match AUTO_BACKEND.get_or_init(detect_auto_backend_kind) {
        Ok(kind) => Ok(*kind),
        Err(message) => bail!("{message}"),
    }
}

fn detect_auto_backend_kind() -> Result<BackendKind, String> {
    if awww::BACKEND.is_available() {
        Ok(BackendKind::Awww)
    } else {
        Err(
            "No supported wallpaper backend found. Install awww or set [backend].kind explicitly."
                .to_string(),
        )
    }
}

fn command_exists(command: &str) -> bool {
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };

    std::env::split_paths(&paths).any(|dir| {
        windows_command_names(command)
            .into_iter()
            .map(|name| dir.join(name))
            .any(|candidate| is_executable_file(&candidate))
    })
}

#[cfg(unix)]
fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;

    path.metadata()
        .map(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
        .unwrap_or(false)
}

#[cfg(not(unix))]
fn is_executable_file(path: &Path) -> bool {
    path.is_file()
}

#[cfg(windows)]
fn windows_command_names(command: &str) -> [OsString; 2] {
    [
        OsString::from(command),
        OsString::from(format!("{command}.exe")),
    ]
}

#[cfg(not(windows))]
fn windows_command_names(command: &str) -> [OsString; 1] {
    [OsString::from(command)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backend_kind_defaults_to_auto() {
        assert_eq!(BackendKind::default(), BackendKind::Auto);
    }

    #[test]
    fn fill_color_hex_is_rgba() {
        let color = FillColor {
            r: 0x12,
            g: 0x34,
            b: 0x56,
            a: 0x78,
        };
        assert_eq!(color.to_hex(), "12345678");
    }

    #[test]
    fn backend_display_names_are_stable() {
        assert_eq!(BackendKind::Auto.display_name(), "auto");
        assert_eq!(BackendKind::Awww.display_name(), "awww");
    }
}
