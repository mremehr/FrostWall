//! Intelligent wallpaper pairing based on usage history
//!
//! Tracks which wallpapers are set together on multi-monitor setups
//! and suggests/auto-applies matching wallpapers based on learned patterns.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

/// A record of wallpapers set together at a point in time
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PairingRecord {
    /// Wallpaper paths for each screen (screen_name -> wallpaper_path)
    pub wallpapers: HashMap<String, PathBuf>,
    /// When this pairing was applied (Unix timestamp)
    pub timestamp: u64,
    /// How long this pairing was kept (seconds), if known
    #[serde(default)]
    pub duration: Option<u64>,
    /// Was it manually selected or auto-applied?
    pub manual: bool,
}

/// Affinity score between two wallpapers
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AffinityScore {
    pub wallpaper_a: PathBuf,
    pub wallpaper_b: PathBuf,
    /// Combined affinity score (higher = better match)
    pub score: f32,
    /// How many times they've been paired together
    pub pair_count: u32,
    /// Average duration when paired (seconds)
    pub avg_duration_secs: f32,
}

/// Persistent pairing history and affinity cache
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct PairingHistoryData {
    pub records: Vec<PairingRecord>,
    pub affinity_scores: Vec<AffinityScore>,
}

/// Runtime state for undo functionality
pub struct UndoState {
    pub previous_wallpapers: HashMap<String, PathBuf>,
    pub started_at: Instant,
    pub duration: Duration,
    pub message: String,
}

/// Manages pairing history and suggestions
pub struct PairingHistory {
    data: PairingHistoryData,
    cache_path: PathBuf,
    /// Current active pairing (for duration tracking)
    current_pairing_start: Option<u64>,
    /// Undo state
    undo_state: Option<UndoState>,
    /// Maximum records to keep
    max_records: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum PairingStyleMode {
    Off,
    #[default]
    Soft,
    Strict,
}

impl PairingStyleMode {
    /// Cycle to the next pairing style mode.
    pub fn next(self) -> Self {
        match self {
            PairingStyleMode::Off => PairingStyleMode::Soft,
            PairingStyleMode::Soft => PairingStyleMode::Strict,
            PairingStyleMode::Strict => PairingStyleMode::Off,
        }
    }

    /// Return human-readable display name for this style mode.
    pub fn display_name(self) -> &'static str {
        match self {
            PairingStyleMode::Off => "Off",
            PairingStyleMode::Soft => "Soft",
            PairingStyleMode::Strict => "Strict",
        }
    }
}

pub struct MatchContext<'a> {
    pub selected_wp: &'a Path,
    pub target_screen: &'a str,
    pub selected_colors: &'a [String],
    pub selected_weights: &'a [f32],
    pub selected_tags: &'a [String],
    pub selected_embedding: Option<&'a [f32]>,
    pub screen_context_weight: f32,
    pub visual_weight: f32,
    pub harmony_weight: f32,
    pub tag_weight: f32,
    pub semantic_weight: f32,
    pub repetition_penalty_weight: f32,
    pub style_mode: PairingStyleMode,
    pub selected_style_tags: &'a [String],
}

mod history;
mod scoring;
mod style_tags;

pub use style_tags::extract_style_tags;

#[cfg(test)]
use scoring::{compare_scored_match, normalize_cosine_similarity};
#[cfg(test)]
use style_tags::{canonical_style_tag, is_content_tag};

#[cfg(test)]
mod tests {
    use super::*;

    // --- PairingStyleMode ---

    #[test]
    fn test_pairing_style_mode_cycle() {
        assert_eq!(PairingStyleMode::Off.next(), PairingStyleMode::Soft);
        assert_eq!(PairingStyleMode::Soft.next(), PairingStyleMode::Strict);
        assert_eq!(PairingStyleMode::Strict.next(), PairingStyleMode::Off);
    }

    #[test]
    fn test_pairing_style_mode_display_name() {
        assert_eq!(PairingStyleMode::Off.display_name(), "Off");
        assert_eq!(PairingStyleMode::Soft.display_name(), "Soft");
        assert_eq!(PairingStyleMode::Strict.display_name(), "Strict");
    }

    #[test]
    fn test_pairing_style_mode_default() {
        assert_eq!(PairingStyleMode::default(), PairingStyleMode::Soft);
    }

    // --- canonical_style_tag ---

    #[test]
    fn test_canonical_style_tag_pixel_art_variants() {
        assert_eq!(canonical_style_tag("8bit"), Some("pixel_art"));
        assert_eq!(canonical_style_tag("8_bit"), Some("pixel_art"));
        assert_eq!(canonical_style_tag("pixelart"), Some("pixel_art"));
        assert_eq!(canonical_style_tag("pixel_art"), Some("pixel_art"));
    }

    #[test]
    fn test_canonical_style_tag_digital_art_variants() {
        assert_eq!(canonical_style_tag("digital_painting"), Some("digital_art"));
        assert_eq!(canonical_style_tag("digitalpainting"), Some("digital_art"));
        assert_eq!(canonical_style_tag("digital_art"), Some("digital_art"));
        assert_eq!(canonical_style_tag("digitalart"), Some("digital_art"));
    }

    #[test]
    fn test_canonical_style_tag_painting_variants() {
        assert_eq!(canonical_style_tag("painted"), Some("painting"));
        assert_eq!(canonical_style_tag("painting"), Some("painting"));
        assert_eq!(canonical_style_tag("painterly"), Some("painting"));
    }

    #[test]
    fn test_canonical_style_tag_illustration_variants() {
        assert_eq!(canonical_style_tag("illustrated"), Some("illustration"));
        assert_eq!(canonical_style_tag("illustration"), Some("illustration"));
    }

    #[test]
    fn test_canonical_style_tag_direct_matches() {
        assert_eq!(canonical_style_tag("anime"), Some("anime"));
        assert_eq!(canonical_style_tag("retro"), Some("retro"));
        assert_eq!(canonical_style_tag("vintage"), Some("vintage"));
        assert_eq!(canonical_style_tag("abstract"), Some("abstract"));
        assert_eq!(canonical_style_tag("geometric"), Some("geometric"));
    }

    #[test]
    fn test_canonical_style_tag_not_style() {
        assert_eq!(canonical_style_tag("nature"), None);
        assert_eq!(canonical_style_tag("ocean"), None);
        assert_eq!(canonical_style_tag("dark"), None);
        assert_eq!(canonical_style_tag("bright"), None);
    }

    #[test]
    fn test_canonical_style_tag_normalization() {
        // Hyphens and spaces should be normalized to underscores
        assert_eq!(canonical_style_tag("pixel-art"), Some("pixel_art"));
        assert_eq!(canonical_style_tag("pixel art"), Some("pixel_art"));
        // Trimming
        assert_eq!(canonical_style_tag("  anime  "), Some("anime"));
        // Case insensitivity
        assert_eq!(canonical_style_tag("ANIME"), Some("anime"));
        assert_eq!(canonical_style_tag("Retro"), Some("retro"));
    }

    // --- extract_style_tags ---

    #[test]
    fn test_extract_style_tags_filters() {
        let tags: Vec<String> = vec![
            "anime".into(),
            "nature".into(),
            "dark".into(),
            "pixel_art".into(),
        ];
        let styles = extract_style_tags(&tags);
        assert!(styles.contains(&"anime".to_string()));
        assert!(styles.contains(&"pixel_art".to_string()));
        assert!(!styles.contains(&"nature".to_string()));
        assert!(!styles.contains(&"dark".to_string()));
    }

    #[test]
    fn test_extract_style_tags_deduplicates() {
        let tags: Vec<String> = vec!["8bit".into(), "pixel_art".into(), "pixelart".into()];
        let styles = extract_style_tags(&tags);
        assert_eq!(styles.len(), 1, "All variants should map to pixel_art");
        assert_eq!(styles[0], "pixel_art");
    }

    #[test]
    fn test_extract_style_tags_empty() {
        let tags: Vec<String> = vec![];
        let styles = extract_style_tags(&tags);
        assert!(styles.is_empty());
    }

    // --- PairingHistory ---

    #[test]
    fn test_pairing_history_new_empty() {
        let history = PairingHistory::new(100);
        assert_eq!(history.record_count(), 0);
        assert_eq!(history.affinity_count(), 0);
        assert!(!history.can_undo());
    }

    fn test_pairing_wallpaper(
        path: &str,
        colors: &[&str],
        tags: &[&str],
    ) -> crate::wallpaper::Wallpaper {
        crate::wallpaper::Wallpaper {
            path: PathBuf::from(path),
            width: 1920,
            height: 1080,
            aspect_category: crate::screen::AspectCategory::Landscape,
            colors: colors.iter().map(|c| (*c).to_string()).collect(),
            color_weights: if colors.is_empty() {
                Vec::new()
            } else {
                vec![1.0 / colors.len() as f32; colors.len()]
            },
            tags: tags.iter().map(|t| (*t).to_string()).collect(),
            auto_tags: Vec::new(),
            embedding: None,
            file_size: 0,
            modified_at: 0,
        }
    }

    #[test]
    fn test_get_top_matches_prefers_higher_history_affinity() {
        let selected_path = PathBuf::from("/test/selected.jpg");
        let high_path = PathBuf::from("/test/high.jpg");
        let low_path = PathBuf::from("/test/low.jpg");

        let history = PairingHistory {
            data: PairingHistoryData {
                records: Vec::new(),
                affinity_scores: vec![
                    AffinityScore {
                        wallpaper_a: selected_path.clone(),
                        wallpaper_b: high_path.clone(),
                        score: 0.95,
                        pair_count: 8,
                        avg_duration_secs: 1200.0,
                    },
                    AffinityScore {
                        wallpaper_a: selected_path.clone(),
                        wallpaper_b: low_path.clone(),
                        score: 0.20,
                        pair_count: 2,
                        avg_duration_secs: 300.0,
                    },
                ],
            },
            cache_path: PathBuf::from("/tmp/frostwall/test_pairing_history.json"),
            current_pairing_start: None,
            undo_state: None,
            max_records: 100,
        };

        let high = test_pairing_wallpaper("/test/high.jpg", &["#223344"], &[]);
        let low = test_pairing_wallpaper("/test/low.jpg", &["#223344"], &[]);
        let selected_colors = vec!["#112233".to_string()];
        let selected_weights = vec![1.0];
        let selected_tags: Vec<String> = Vec::new();
        let selected_style_tags: Vec<String> = Vec::new();

        let context = MatchContext {
            selected_wp: &selected_path,
            target_screen: "DP-2",
            selected_colors: &selected_colors,
            selected_weights: &selected_weights,
            selected_tags: &selected_tags,
            selected_embedding: None,
            screen_context_weight: 1.0,
            visual_weight: 0.0,
            harmony_weight: 0.0,
            tag_weight: 0.0,
            semantic_weight: 0.0,
            repetition_penalty_weight: 0.0,
            style_mode: PairingStyleMode::Off,
            selected_style_tags: &selected_style_tags,
        };

        let matches = history.get_top_matches(&context, &[&low, &high], 2);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].0, high_path);
        assert_eq!(matches[1].0, low_path);
        assert!(matches[0].1 > matches[1].1);
    }

    #[test]
    fn test_get_top_matches_strict_filters_style_mismatch_even_with_high_history() {
        let selected_path = PathBuf::from("/test/selected.jpg");
        let anime_path = PathBuf::from("/test/anime.jpg");
        let photo_path = PathBuf::from("/test/photo.jpg");

        let history = PairingHistory {
            data: PairingHistoryData {
                records: Vec::new(),
                affinity_scores: vec![
                    // Wrong style has stronger history, but strict mode should still reject it.
                    AffinityScore {
                        wallpaper_a: selected_path.clone(),
                        wallpaper_b: photo_path.clone(),
                        score: 0.99,
                        pair_count: 10,
                        avg_duration_secs: 2400.0,
                    },
                    AffinityScore {
                        wallpaper_a: selected_path.clone(),
                        wallpaper_b: anime_path.clone(),
                        score: 0.10,
                        pair_count: 1,
                        avg_duration_secs: 120.0,
                    },
                ],
            },
            cache_path: PathBuf::from("/tmp/frostwall/test_pairing_history_strict.json"),
            current_pairing_start: None,
            undo_state: None,
            max_records: 100,
        };

        let anime = test_pairing_wallpaper("/test/anime.jpg", &["#112233"], &["anime"]);
        let photo = test_pairing_wallpaper("/test/photo.jpg", &["#112233"], &["photography"]);
        let selected_colors = vec!["#112233".to_string()];
        let selected_weights = vec![1.0];
        let selected_tags = vec!["anime".to_string()];
        let selected_style_tags = extract_style_tags(&selected_tags);

        let context = MatchContext {
            selected_wp: &selected_path,
            target_screen: "DP-2",
            selected_colors: &selected_colors,
            selected_weights: &selected_weights,
            selected_tags: &selected_tags,
            selected_embedding: None,
            screen_context_weight: 1.0,
            visual_weight: 1.0,
            harmony_weight: 0.0,
            tag_weight: 1.0,
            semantic_weight: 0.0,
            repetition_penalty_weight: 0.0,
            style_mode: PairingStyleMode::Strict,
            selected_style_tags: &selected_style_tags,
        };

        let matches = history.get_top_matches(&context, &[&photo, &anime], 5);
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].0, anime_path);
    }

    // --- normalize_cosine_similarity ---

    #[test]
    fn test_normalize_cosine_similarity_identical() {
        let v = vec![1.0, 0.0, 0.0];
        let result = normalize_cosine_similarity(&v, &v);
        assert!(
            (result - 1.0).abs() < 0.001,
            "Identical vectors should have similarity ~1.0, got {}",
            result
        );
    }

    #[test]
    fn test_normalize_cosine_similarity_opposite() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![-1.0, 0.0, 0.0];
        let result = normalize_cosine_similarity(&a, &b);
        assert!(
            result.abs() < 0.001,
            "Opposite vectors should have similarity ~0.0, got {}",
            result
        );
    }

    #[test]
    fn test_normalize_cosine_similarity_orthogonal() {
        let a = vec![1.0, 0.0, 0.0];
        let b = vec![0.0, 1.0, 0.0];
        let result = normalize_cosine_similarity(&a, &b);
        assert!(
            (result - 0.5).abs() < 0.001,
            "Orthogonal vectors should have similarity ~0.5, got {}",
            result
        );
    }

    #[test]
    fn test_normalize_cosine_similarity_empty() {
        assert_eq!(normalize_cosine_similarity(&[], &[]), 0.0);
    }

    // --- is_content_tag ---

    #[test]
    fn test_is_content_tag() {
        assert!(is_content_tag("nature"));
        assert!(is_content_tag("ocean"));
        assert!(is_content_tag("forest"));
        assert!(!is_content_tag("bright")); // mood, not content
        assert!(!is_content_tag("dark"));
        assert!(!is_content_tag("anime")); // style tag
        assert!(!is_content_tag("pixel_art")); // style tag
    }

    // --- compare_scored_match ---

    #[test]
    fn test_compare_scored_match_by_score() {
        let a = (PathBuf::from("a.jpg"), 0.8);
        let b = (PathBuf::from("b.jpg"), 0.9);
        // Higher score should come first (b before a)
        assert_eq!(compare_scored_match(&a, &b), std::cmp::Ordering::Greater);
        assert_eq!(compare_scored_match(&b, &a), std::cmp::Ordering::Less);
    }

    #[test]
    fn test_compare_scored_match_tiebreak_by_path() {
        let a = (PathBuf::from("a.jpg"), 0.8);
        let b = (PathBuf::from("b.jpg"), 0.8);
        // Equal scores should sort by path
        assert_eq!(compare_scored_match(&a, &b), std::cmp::Ordering::Less);
    }
}
