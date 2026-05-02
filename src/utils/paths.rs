use directories::{BaseDirs, ProjectDirs, UserDirs};
use std::borrow::Cow;
use std::path::{Path, PathBuf};

const PROJECT_QUALIFIER: &str = "com";
const PROJECT_ORGANIZATION: &str = "mrmattias";
const PROJECT_APPLICATION: &str = "frostwall";

fn project_dirs() -> Option<ProjectDirs> {
    ProjectDirs::from(PROJECT_QUALIFIER, PROJECT_ORGANIZATION, PROJECT_APPLICATION)
}

/// User's home directory, when discoverable on this platform.
pub fn home_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|d| d.home_dir().to_path_buf())
}

/// Platform cache directory (parent of project cache), when discoverable.
pub fn cache_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|d| d.cache_dir().to_path_buf())
}

/// User's pictures directory, when configured by the desktop environment.
pub fn picture_dir() -> Option<PathBuf> {
    UserDirs::new().and_then(|d| d.picture_dir().map(Path::to_path_buf))
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
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    std::path::PathBuf::from(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_image_file_supported_extensions() {
        assert!(is_image_file(Path::new("photo.jpg")));
        assert!(is_image_file(Path::new("photo.jpeg")));
        assert!(is_image_file(Path::new("photo.png")));
        assert!(is_image_file(Path::new("photo.webp")));
        assert!(is_image_file(Path::new("photo.bmp")));
        assert!(is_image_file(Path::new("photo.gif")));
    }

    #[test]
    fn is_image_file_case_insensitive() {
        assert!(is_image_file(Path::new("photo.JPG")));
        assert!(is_image_file(Path::new("photo.PNG")));
        assert!(is_image_file(Path::new("photo.WebP")));
    }

    #[test]
    fn is_image_file_rejects_other_extensions() {
        assert!(!is_image_file(Path::new("document.txt")));
        assert!(!is_image_file(Path::new("video.mp4")));
        assert!(!is_image_file(Path::new("noextension")));
        assert!(!is_image_file(Path::new(".hidden")));
    }

    #[test]
    fn expand_tilde_replaces_prefix() {
        let expanded = expand_tilde("~/documents/test.png");
        let s = expanded.to_string_lossy();
        assert!(!s.starts_with("~/"), "got {s}");
        assert!(s.ends_with("documents/test.png"));
    }

    #[test]
    fn expand_tilde_leaves_absolute_path_unchanged() {
        let path = "/absolute/path/file.png";
        assert_eq!(expand_tilde(path).to_string_lossy(), path);
    }
}
