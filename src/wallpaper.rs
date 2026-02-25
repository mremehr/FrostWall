use crate::clip::AutoTag;
use crate::screen::AspectCategory;
#[cfg(test)]
use crate::screen::Screen;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// How strictly to match wallpaper aspect ratio to screen
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum MatchMode {
    /// Only exact aspect category match
    #[default]
    Strict,
    /// Flexible: landscape works on ultrawide, portrait on portrait
    Flexible,
    /// Show all wallpapers regardless of aspect ratio
    All,
}

/// Sort order for wallpapers
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum SortMode {
    /// Sort by filename (A-Z)
    #[default]
    Name,
    /// Sort by image dimensions (largest first)
    Size,
    /// Sort by modification date (newest first)
    Date,
}

impl SortMode {
    /// Return human-readable display name for this sort mode.
    pub fn display_name(&self) -> &'static str {
        match self {
            SortMode::Name => "Name",
            SortMode::Size => "Size",
            SortMode::Date => "Date",
        }
    }

    /// Cycle to the next sort mode.
    pub fn next(&self) -> Self {
        match self {
            SortMode::Name => SortMode::Size,
            SortMode::Size => SortMode::Date,
            SortMode::Date => SortMode::Name,
        }
    }
}

impl MatchMode {
    /// Return human-readable display name for this match mode.
    pub fn display_name(&self) -> &'static str {
        match self {
            MatchMode::Strict => "Strict",
            MatchMode::Flexible => "Flexible",
            MatchMode::All => "All",
        }
    }

    /// Cycle to the next match mode.
    pub fn next(&self) -> Self {
        match self {
            MatchMode::Strict => MatchMode::Flexible,
            MatchMode::Flexible => MatchMode::All,
            MatchMode::All => MatchMode::Strict,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Wallpaper {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
    pub aspect_category: AspectCategory,
    pub colors: Vec<String>,
    /// User-defined tags for this wallpaper
    #[serde(default)]
    pub tags: Vec<String>,
    /// CLIP-generated auto tags with confidence scores
    #[serde(default)]
    pub auto_tags: Vec<AutoTag>,
    /// Color weights/proportions (how much of the image each color represents, 0.0-1.0)
    #[serde(default)]
    pub color_weights: Vec<f32>,
    /// Cached CLIP embedding for similarity search (512 dimensions)
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
    /// File size in bytes (for sorting)
    #[serde(default)]
    pub file_size: u64,
    /// Modification timestamp (seconds since epoch, for sorting)
    #[serde(default)]
    pub modified_at: u64,
}

/// Current cache format version — bump when the serialized shape changes
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy)]
pub enum CacheLoadMode {
    Full,
    MetadataOnly,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallpaperCache {
    /// Cache format version for forward-compatible migration
    #[serde(default)]
    pub version: u32,
    pub wallpapers: Vec<Wallpaper>,
    pub source_dir: PathBuf,
    /// Track current index per screen for next/prev
    #[serde(default)]
    pub screen_indices: HashMap<String, usize>,
    /// Whether the cache was built with recursive scanning
    #[serde(default)]
    pub recursive: bool,
}

#[derive(Debug, Default)]
pub struct CacheStats {
    pub total: usize,
    pub ultrawide: usize,
    pub landscape: usize,
    pub portrait: usize,
    pub square: usize,
}

mod cache;
mod model;

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a minimal Wallpaper for testing without filesystem access
    fn test_wallpaper(width: u32, height: u32) -> Wallpaper {
        Wallpaper {
            path: PathBuf::from("/test/fake.jpg"),
            width,
            height,
            aspect_category: Wallpaper::categorize_aspect(width, height),
            colors: vec![],
            color_weights: vec![],
            tags: vec![],
            auto_tags: vec![],
            embedding: None,
            file_size: 0,
            modified_at: 0,
        }
    }

    fn named_wallpaper(name: &str, width: u32, height: u32) -> Wallpaper {
        Wallpaper {
            path: PathBuf::from(format!("/test/{name}.jpg")),
            width,
            height,
            aspect_category: Wallpaper::categorize_aspect(width, height),
            colors: vec![],
            color_weights: vec![],
            tags: vec![],
            auto_tags: vec![],
            embedding: None,
            file_size: 0,
            modified_at: 0,
        }
    }

    // --- categorize_aspect ---

    #[test]
    fn test_categorize_aspect_ultrawide() {
        assert_eq!(
            Wallpaper::categorize_aspect(3840, 1080),
            AspectCategory::Ultrawide
        ); // 3.56
        assert_eq!(
            Wallpaper::categorize_aspect(5120, 1440),
            AspectCategory::Ultrawide
        ); // 3.56
        assert_eq!(
            Wallpaper::categorize_aspect(2560, 1080),
            AspectCategory::Ultrawide
        ); // 2.37
    }

    #[test]
    fn test_categorize_aspect_landscape() {
        assert_eq!(
            Wallpaper::categorize_aspect(1920, 1080),
            AspectCategory::Landscape
        ); // 1.78
        assert_eq!(
            Wallpaper::categorize_aspect(1920, 1200),
            AspectCategory::Landscape
        ); // 1.6
        assert_eq!(
            Wallpaper::categorize_aspect(2560, 1440),
            AspectCategory::Landscape
        ); // 1.78
    }

    #[test]
    fn test_categorize_aspect_portrait() {
        assert_eq!(
            Wallpaper::categorize_aspect(1080, 1920),
            AspectCategory::Portrait
        );
        assert_eq!(
            Wallpaper::categorize_aspect(1440, 2560),
            AspectCategory::Portrait
        );
    }

    #[test]
    fn test_categorize_aspect_square() {
        assert_eq!(
            Wallpaper::categorize_aspect(1000, 1000),
            AspectCategory::Square
        );
        assert_eq!(
            Wallpaper::categorize_aspect(1100, 1000),
            AspectCategory::Square
        ); // 1.1 < 1.2
    }

    #[test]
    fn test_categorize_aspect_boundary_landscape_square() {
        // Exactly 1.2 ratio should be Landscape
        assert_eq!(
            Wallpaper::categorize_aspect(1200, 1000),
            AspectCategory::Landscape
        );
        // Just below 1.2 should be Square
        assert_eq!(
            Wallpaper::categorize_aspect(1199, 1000),
            AspectCategory::Square
        );
    }

    #[test]
    fn test_categorize_aspect_boundary_ultrawide() {
        // Exactly 2.0 ratio should be Ultrawide
        assert_eq!(
            Wallpaper::categorize_aspect(2000, 1000),
            AspectCategory::Ultrawide
        );
        // Just below 2.0 should be Landscape
        assert_eq!(
            Wallpaper::categorize_aspect(1999, 1000),
            AspectCategory::Landscape
        );
    }

    // --- matches_screen ---

    #[test]
    fn test_matches_screen_exact() {
        let wp = test_wallpaper(1920, 1080); // Landscape
        let screen = Screen::new("DP-1".into(), 1920, 1080);
        assert!(wp.matches_screen(&screen));
    }

    #[test]
    fn test_matches_screen_different_category() {
        let wp = test_wallpaper(1920, 1080); // Landscape
        let screen = Screen::new("DP-1".into(), 1080, 1920); // Portrait
        assert!(!wp.matches_screen(&screen));
    }

    // --- matches_screen_flexible ---

    #[test]
    fn test_matches_screen_flexible_landscape_on_ultrawide() {
        let wp = test_wallpaper(1920, 1080); // Landscape
        let screen = Screen::new("DP-1".into(), 5120, 1440); // Ultrawide
        assert!(wp.matches_screen_flexible(&screen));
    }

    #[test]
    fn test_matches_screen_flexible_ultrawide_on_landscape() {
        let wp = test_wallpaper(5120, 1440); // Ultrawide
        let screen = Screen::new("DP-1".into(), 1920, 1080); // Landscape
        assert!(wp.matches_screen_flexible(&screen));
    }

    #[test]
    fn test_matches_screen_flexible_square_versatile() {
        let wp = test_wallpaper(1000, 1000); // Square
        let landscape = Screen::new("DP-1".into(), 1920, 1080);
        let portrait = Screen::new("DP-2".into(), 1080, 1920);
        let ultrawide = Screen::new("DP-3".into(), 5120, 1440);
        assert!(wp.matches_screen_flexible(&landscape));
        assert!(wp.matches_screen_flexible(&portrait));
        assert!(wp.matches_screen_flexible(&ultrawide));
    }

    #[test]
    fn test_matches_screen_flexible_portrait_not_landscape() {
        let wp = test_wallpaper(1080, 1920); // Portrait
        let screen = Screen::new("DP-1".into(), 1920, 1080); // Landscape
        assert!(!wp.matches_screen_flexible(&screen));
    }

    // --- matches_screen_with_mode ---

    #[test]
    fn test_matches_screen_with_mode_all() {
        let wp = test_wallpaper(1080, 1920); // Portrait
        let screen = Screen::new("DP-1".into(), 1920, 1080); // Landscape
        assert!(wp.matches_screen_with_mode(&screen, MatchMode::All));
    }

    // --- add_tag / remove_tag / has_tag ---

    #[test]
    fn test_add_tag() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        assert!(wp.has_tag("nature"));
        assert_eq!(wp.tags.len(), 1);
    }

    #[test]
    fn test_add_tag_duplicate() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("nature");
        assert_eq!(wp.tags.len(), 1, "Duplicate tag should not be added");
    }

    #[test]
    fn test_add_tag_case_insensitive() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("Nature");
        assert!(wp.has_tag("nature"));
        assert!(wp.has_tag("NATURE"));
    }

    #[test]
    fn test_add_tag_empty() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("");
        wp.add_tag("   ");
        assert_eq!(
            wp.tags.len(),
            0,
            "Empty/whitespace tags should not be added"
        );
    }

    #[test]
    fn test_remove_tag() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("forest");
        wp.remove_tag("nature");
        assert!(!wp.has_tag("nature"));
        assert!(wp.has_tag("forest"));
    }

    #[test]
    fn test_has_tag_includes_auto_tags() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.auto_tags.push(AutoTag {
            name: "sunset".into(),
            confidence: 0.9,
        });
        assert!(wp.has_tag("sunset"));
        assert!(wp.has_tag("SUNSET"));
    }

    // --- has_any_tag / has_all_tags ---

    #[test]
    fn test_has_any_tag() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("landscape");
        assert!(wp.has_any_tag(&["nature".into(), "ocean".into()]));
        assert!(!wp.has_any_tag(&["space".into(), "cyberpunk".into()]));
    }

    #[test]
    fn test_has_all_tags() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.add_tag("landscape");
        assert!(wp.has_all_tags(&["nature".into(), "landscape".into()]));
        assert!(!wp.has_all_tags(&["nature".into(), "ocean".into()]));
    }

    // --- all_tags ---

    #[test]
    fn test_all_tags_combines_manual_and_auto() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.auto_tags.push(AutoTag {
            name: "forest".into(),
            confidence: 0.8,
        });
        let all = wp.all_tags();
        assert!(all.contains(&"nature".into()));
        assert!(all.contains(&"forest".into()));
    }

    #[test]
    fn test_all_tags_deduplicates() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.add_tag("nature");
        wp.auto_tags.push(AutoTag {
            name: "nature".into(),
            confidence: 0.8,
        });
        let all = wp.all_tags();
        assert_eq!(all.iter().filter(|t| *t == "nature").count(), 1);
    }

    // --- SortMode / MatchMode cycling ---

    #[test]
    fn test_sort_mode_cycle() {
        assert_eq!(SortMode::Name.next(), SortMode::Size);
        assert_eq!(SortMode::Size.next(), SortMode::Date);
        assert_eq!(SortMode::Date.next(), SortMode::Name);
    }

    #[test]
    fn test_match_mode_cycle() {
        assert_eq!(MatchMode::Strict.next(), MatchMode::Flexible);
        assert_eq!(MatchMode::Flexible.next(), MatchMode::All);
        assert_eq!(MatchMode::All.next(), MatchMode::Strict);
    }

    // --- auto_tags_above ---

    #[test]
    fn test_auto_tags_above_threshold() {
        let mut wp = test_wallpaper(1920, 1080);
        wp.auto_tags.push(AutoTag {
            name: "nature".into(),
            confidence: 0.9,
        });
        wp.auto_tags.push(AutoTag {
            name: "dark".into(),
            confidence: 0.3,
        });
        wp.auto_tags.push(AutoTag {
            name: "forest".into(),
            confidence: 0.7,
        });

        let above = wp.auto_tags_above(0.5);
        assert_eq!(above.len(), 2);
        assert!(above.iter().all(|t| t.confidence >= 0.5));
    }

    // --- primary_color ---

    #[test]
    fn test_primary_color() {
        let mut wp = test_wallpaper(1920, 1080);
        assert!(wp.primary_color().is_none());
        wp.colors = vec!["#FF0000".into(), "#00FF00".into()];
        assert_eq!(wp.primary_color(), Some("#FF0000"));
    }

    #[test]
    fn test_next_for_screen_starts_from_first_matching_wallpaper() {
        let screen = Screen::new("DP-1".into(), 1920, 1080);
        let first = named_wallpaper("first", 1920, 1080);
        let second = named_wallpaper("second", 1920, 1080);

        let mut cache = WallpaperCache {
            version: CACHE_VERSION,
            wallpapers: vec![first, second],
            source_dir: PathBuf::from("/test"),
            screen_indices: HashMap::new(),
            recursive: false,
        };

        let selected = cache
            .next_for_screen(&screen)
            .expect("expected a matching wallpaper");
        assert_eq!(selected.path, PathBuf::from("/test/first.jpg"));
    }

    #[test]
    fn test_prev_for_screen_starts_from_last_matching_wallpaper() {
        let screen = Screen::new("DP-1".into(), 1920, 1080);
        let first = named_wallpaper("first", 1920, 1080);
        let second = named_wallpaper("second", 1920, 1080);

        let mut cache = WallpaperCache {
            version: CACHE_VERSION,
            wallpapers: vec![first, second],
            source_dir: PathBuf::from("/test"),
            screen_indices: HashMap::new(),
            recursive: false,
        };

        let selected = cache
            .prev_for_screen(&screen)
            .expect("expected a matching wallpaper");
        assert_eq!(selected.path, PathBuf::from("/test/second.jpg"));
    }

    #[test]
    fn test_wallpaper_cache_serde_roundtrip_preserves_fields() {
        let mut first = named_wallpaper("first", 1920, 1080);
        first.colors = vec!["#112233".into(), "#445566".into()];
        first.color_weights = vec![0.7, 0.3];
        first.tags = vec!["nature".into(), "forest".into()];
        first.auto_tags = vec![AutoTag {
            name: "mist".into(),
            confidence: 0.91,
        }];
        first.embedding = Some(vec![0.1, 0.2, 0.3]);
        first.file_size = 4242;
        first.modified_at = 1_700_000_000;

        let second = named_wallpaper("second", 2560, 1440);

        let mut cache = WallpaperCache {
            version: CACHE_VERSION,
            wallpapers: vec![first, second],
            source_dir: PathBuf::from("/test"),
            screen_indices: HashMap::new(),
            recursive: true,
        };
        cache.screen_indices.insert("DP-1".into(), 1);

        let json = serde_json::to_string(&cache).expect("serialize cache");
        let decoded: WallpaperCache = serde_json::from_str(&json).expect("deserialize cache");

        assert_eq!(decoded.version, CACHE_VERSION);
        assert!(decoded.recursive);
        assert_eq!(decoded.screen_indices.get("DP-1"), Some(&1usize));
        assert_eq!(decoded.wallpapers.len(), 2);

        let decoded_first = &decoded.wallpapers[0];
        assert_eq!(decoded_first.path, PathBuf::from("/test/first.jpg"));
        assert_eq!(
            decoded_first.colors,
            vec!["#112233".to_string(), "#445566".to_string()]
        );
        assert_eq!(decoded_first.color_weights, vec![0.7, 0.3]);
        assert_eq!(
            decoded_first.tags,
            vec!["nature".to_string(), "forest".to_string()]
        );
        assert_eq!(decoded_first.auto_tags.len(), 1);
        assert_eq!(decoded_first.auto_tags[0].name, "mist");
        assert_eq!(decoded_first.auto_tags[0].confidence, 0.91);
        assert_eq!(decoded_first.embedding, Some(vec![0.1, 0.2, 0.3]));
        assert_eq!(decoded_first.file_size, 4242);
        assert_eq!(decoded_first.modified_at, 1_700_000_000);
    }

    #[test]
    fn test_wallpaper_cache_deserialize_legacy_defaults() {
        let legacy = r##"{
            "wallpapers":[
                {
                    "path":"/test/legacy.jpg",
                    "width":1920,
                    "height":1080,
                    "aspect_category":"Landscape",
                    "colors":["#000000"]
                }
            ],
            "source_dir":"/test",
            "screen_indices":{"DP-1":0}
        }"##;

        let decoded: WallpaperCache =
            serde_json::from_str(legacy).expect("deserialize legacy cache");
        assert_eq!(decoded.version, 0);
        assert!(!decoded.recursive);
        assert_eq!(decoded.screen_indices.get("DP-1"), Some(&0usize));
        assert_eq!(decoded.wallpapers.len(), 1);

        let wp = &decoded.wallpapers[0];
        assert_eq!(wp.path, PathBuf::from("/test/legacy.jpg"));
        assert_eq!(wp.colors, vec!["#000000".to_string()]);
        assert!(wp.color_weights.is_empty());
        assert!(wp.tags.is_empty());
        assert!(wp.auto_tags.is_empty());
        assert!(wp.embedding.is_none());
        assert_eq!(wp.file_size, 0);
        assert_eq!(wp.modified_at, 0);
    }
}
