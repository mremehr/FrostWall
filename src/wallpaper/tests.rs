use super::*;
use crate::screen::Screen;

/// Create a minimal Wallpaper for testing without filesystem access.
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

#[test]
fn test_categorize_aspect_ultrawide() {
    assert_eq!(
        Wallpaper::categorize_aspect(3840, 1080),
        AspectCategory::Ultrawide
    );
    assert_eq!(
        Wallpaper::categorize_aspect(5120, 1440),
        AspectCategory::Ultrawide
    );
    assert_eq!(
        Wallpaper::categorize_aspect(2560, 1080),
        AspectCategory::Ultrawide
    );
}

#[test]
fn test_categorize_aspect_landscape() {
    assert_eq!(
        Wallpaper::categorize_aspect(1920, 1080),
        AspectCategory::Landscape
    );
    assert_eq!(
        Wallpaper::categorize_aspect(1920, 1200),
        AspectCategory::Landscape
    );
    assert_eq!(
        Wallpaper::categorize_aspect(2560, 1440),
        AspectCategory::Landscape
    );
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
    );
}

#[test]
fn test_categorize_aspect_boundary_landscape_square() {
    assert_eq!(
        Wallpaper::categorize_aspect(1200, 1000),
        AspectCategory::Landscape
    );
    assert_eq!(
        Wallpaper::categorize_aspect(1199, 1000),
        AspectCategory::Square
    );
}

#[test]
fn test_categorize_aspect_boundary_ultrawide() {
    assert_eq!(
        Wallpaper::categorize_aspect(2000, 1000),
        AspectCategory::Ultrawide
    );
    assert_eq!(
        Wallpaper::categorize_aspect(1999, 1000),
        AspectCategory::Landscape
    );
}

#[test]
fn test_matches_screen_exact() {
    let wallpaper = test_wallpaper(1920, 1080);
    let screen = Screen::new("DP-1".into(), 1920, 1080);
    assert!(wallpaper.matches_screen(&screen));
}

#[test]
fn test_matches_screen_different_category() {
    let wallpaper = test_wallpaper(1920, 1080);
    let screen = Screen::new("DP-1".into(), 1080, 1920);
    assert!(!wallpaper.matches_screen(&screen));
}

#[test]
fn test_matches_screen_flexible_landscape_on_ultrawide() {
    let wallpaper = test_wallpaper(1920, 1080);
    let screen = Screen::new("DP-1".into(), 5120, 1440);
    assert!(wallpaper.matches_screen_flexible(&screen));
}

#[test]
fn test_matches_screen_flexible_ultrawide_on_landscape() {
    let wallpaper = test_wallpaper(5120, 1440);
    let screen = Screen::new("DP-1".into(), 1920, 1080);
    assert!(wallpaper.matches_screen_flexible(&screen));
}

#[test]
fn test_matches_screen_flexible_square_versatile() {
    let wallpaper = test_wallpaper(1000, 1000);
    let landscape = Screen::new("DP-1".into(), 1920, 1080);
    let portrait = Screen::new("DP-2".into(), 1080, 1920);
    let ultrawide = Screen::new("DP-3".into(), 5120, 1440);
    assert!(wallpaper.matches_screen_flexible(&landscape));
    assert!(wallpaper.matches_screen_flexible(&portrait));
    assert!(wallpaper.matches_screen_flexible(&ultrawide));
}

#[test]
fn test_matches_screen_flexible_portrait_not_landscape() {
    let wallpaper = test_wallpaper(1080, 1920);
    let screen = Screen::new("DP-1".into(), 1920, 1080);
    assert!(!wallpaper.matches_screen_flexible(&screen));
}

#[test]
fn test_matches_screen_with_mode_all() {
    let wallpaper = test_wallpaper(1080, 1920);
    let screen = Screen::new("DP-1".into(), 1920, 1080);
    assert!(wallpaper.matches_screen_with_mode(&screen, MatchMode::All));
}

#[test]
fn test_add_tag() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("nature");
    assert!(wallpaper.has_tag("nature"));
    assert_eq!(wallpaper.tags.len(), 1);
}

#[test]
fn test_add_tag_duplicate() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("nature");
    wallpaper.add_tag("nature");
    assert_eq!(wallpaper.tags.len(), 1, "Duplicate tag should not be added");
}

#[test]
fn test_add_tag_case_insensitive() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("Nature");
    assert!(wallpaper.has_tag("nature"));
    assert!(wallpaper.has_tag("NATURE"));
}

#[test]
fn test_add_tag_empty() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("");
    wallpaper.add_tag("   ");
    assert_eq!(wallpaper.tags.len(), 0);
}

#[test]
fn test_remove_tag() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("nature");
    wallpaper.add_tag("forest");
    wallpaper.remove_tag("nature");
    assert!(!wallpaper.has_tag("nature"));
    assert!(wallpaper.has_tag("forest"));
}

#[test]
fn test_has_tag_includes_auto_tags() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.auto_tags.push(AutoTag {
        name: "sunset".into(),
        confidence: 0.9,
    });
    assert!(wallpaper.has_tag("sunset"));
    assert!(wallpaper.has_tag("SUNSET"));
}

#[test]
fn test_has_any_tag() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("nature");
    wallpaper.add_tag("landscape");
    assert!(wallpaper.has_any_tag(&["nature".into(), "ocean".into()]));
    assert!(!wallpaper.has_any_tag(&["space".into(), "cyberpunk".into()]));
}

#[test]
fn test_has_all_tags() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("nature");
    wallpaper.add_tag("landscape");
    assert!(wallpaper.has_all_tags(&["nature".into(), "landscape".into()]));
    assert!(!wallpaper.has_all_tags(&["nature".into(), "ocean".into()]));
}

#[test]
fn test_all_tags_combines_manual_and_auto() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("nature");
    wallpaper.auto_tags.push(AutoTag {
        name: "forest".into(),
        confidence: 0.8,
    });
    let all_tags = wallpaper.all_tags();
    assert!(all_tags.contains(&"nature".into()));
    assert!(all_tags.contains(&"forest".into()));
}

#[test]
fn test_all_tags_deduplicates() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.add_tag("nature");
    wallpaper.auto_tags.push(AutoTag {
        name: "nature".into(),
        confidence: 0.8,
    });
    let all_tags = wallpaper.all_tags();
    assert_eq!(all_tags.iter().filter(|tag| *tag == "nature").count(), 1);
}

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

#[test]
fn test_auto_tags_above_threshold() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    wallpaper.auto_tags.push(AutoTag {
        name: "nature".into(),
        confidence: 0.9,
    });
    wallpaper.auto_tags.push(AutoTag {
        name: "dark".into(),
        confidence: 0.3,
    });
    wallpaper.auto_tags.push(AutoTag {
        name: "forest".into(),
        confidence: 0.7,
    });

    let above = wallpaper.auto_tags_above(0.5);
    assert_eq!(above.len(), 2);
    assert!(above.iter().all(|tag| tag.confidence >= 0.5));
}

#[test]
fn test_primary_color() {
    let mut wallpaper = test_wallpaper(1920, 1080);
    assert!(wallpaper.primary_color().is_none());
    wallpaper.colors = vec!["#FF0000".into(), "#00FF00".into()];
    assert_eq!(wallpaper.primary_color(), Some("#FF0000"));
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
        screen_match_indices: HashMap::new(),
        similarity_profiles: Vec::new(),
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
        screen_match_indices: HashMap::new(),
        similarity_profiles: Vec::new(),
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
        screen_match_indices: HashMap::new(),
        similarity_profiles: Vec::new(),
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

    let decoded: WallpaperCache = serde_json::from_str(legacy).expect("deserialize legacy cache");
    assert_eq!(decoded.version, 0);
    assert!(!decoded.recursive);
    assert_eq!(decoded.screen_indices.get("DP-1"), Some(&0usize));
    assert_eq!(decoded.wallpapers.len(), 1);

    let wallpaper = &decoded.wallpapers[0];
    assert_eq!(wallpaper.path, PathBuf::from("/test/legacy.jpg"));
    assert_eq!(wallpaper.colors, vec!["#000000".to_string()]);
    assert!(wallpaper.color_weights.is_empty());
    assert!(wallpaper.tags.is_empty());
    assert!(wallpaper.auto_tags.is_empty());
    assert!(wallpaper.embedding.is_none());
    assert_eq!(wallpaper.file_size, 0);
    assert_eq!(wallpaper.modified_at, 0);
}
