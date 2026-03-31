use directories::ProjectDirs;
use std::borrow::Cow;
use std::path::{Path, PathBuf};

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "mrmattias";
const PROJECT_APPLICATION: &str = "frostwall";

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION)
}

/// Supported image file extensions
pub const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "bmp", "gif"];

/// Resolve the application's cache directory, or fall back when platform dirs
/// are unavailable.
pub fn project_cache_dir(fallback: impl Into<PathBuf>) -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.cache_dir().to_path_buf())
        .unwrap_or_else(|| fallback.into())
}

/// Resolve the application's config directory, or fall back when platform dirs
/// are unavailable.
pub fn project_config_dir(fallback: impl Into<PathBuf>) -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.config_dir().to_path_buf())
        .unwrap_or_else(|| fallback.into())
}

/// Resolve the application's data directory, or fall back when platform dirs
/// are unavailable.
pub fn project_data_dir(fallback: impl Into<PathBuf>) -> PathBuf {
    project_dirs()
        .map(|dirs| dirs.data_dir().to_path_buf())
        .unwrap_or_else(|| fallback.into())
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

/// Return a friendly display label for a path, preferring the basename.
pub fn display_path_name(path: &Path) -> Cow<'_, str> {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(Cow::Borrowed)
        .unwrap_or_else(|| path.to_string_lossy())
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
