use std::collections::HashSet;

const STYLE_TAGS: &[&str] = &[
    "3d_render",
    "abstract",
    "anime",
    "anime_character",
    "art_nouveau",
    "chibi",
    "concept_art",
    "cyberpunk",
    "digital_art",
    "fantasy",
    "fantasy_landscape",
    "geometric",
    "gothic",
    "illustration",
    "line_art",
    "mecha",
    "moody_fantasy",
    "oil_painting",
    "painting",
    "photography",
    "pixel_art",
    "retro",
    "sci_fi",
    "shoujo",
    "steampunk",
    "vaporwave",
    "vintage",
    "watercolor",
];

pub(crate) fn canonical_style_tag(tag: &str) -> Option<&'static str> {
    let normalized = tag
        .trim()
        .to_lowercase()
        .replace(['-', ' '], "_")
        .trim_matches('_')
        .to_string();
    match normalized.as_str() {
        "8bit" | "8_bit" | "pixelart" | "pixel_art" => Some("pixel_art"),
        "anime_character" | "animecharacter" => Some("anime_character"),
        "concept_art" | "conceptart" => Some("concept_art"),
        "digital_painting" | "digital_art" | "digitalpainting" | "digitalart" => {
            Some("digital_art")
        }
        "line_art" | "lineart" => Some("line_art"),
        "fantasy_landscape" | "fantasylandscape" => Some("fantasy_landscape"),
        "moody_fantasy" | "moodyfantasy" => Some("moody_fantasy"),
        "painted" | "painting" | "painterly" => Some("painting"),
        "illustrated" | "illustration" => Some("illustration"),
        "3d" | "3d_render" | "3d_art" | "cgi" => Some("3d_render"),
        "oil_painting" | "oilpainting" | "oil" => Some("oil_painting"),
        "watercolor" | "watercolour" | "aquarelle" => Some("watercolor"),
        "art_nouveau" | "artnouveau" => Some("art_nouveau"),
        "vaporwave" | "vapor_wave" => Some("vaporwave"),
        "steampunk" | "steam_punk" => Some("steampunk"),
        "shoujo" | "shojo" => Some("shoujo"),
        "mech" | "mecha" | "robot" => Some("mecha"),
        "sci_fi" | "scifi" | "science_fiction" => Some("sci_fi"),
        "photo" | "photograph" | "photography" => Some("photography"),
        other => STYLE_TAGS.iter().copied().find(|style| *style == other),
    }
}

/// Extract and canonicalize style tags from a list of tags.
pub fn extract_style_tags(tags: &[String]) -> Vec<String> {
    let mut styles: Vec<String> = tags
        .iter()
        .filter_map(|tag| canonical_style_tag(tag))
        .map(str::to_string)
        .collect();
    styles.sort();
    styles.dedup();
    styles
}

pub(super) fn collect_style_tags<'a>(tags: impl Iterator<Item = &'a str>) -> HashSet<&'static str> {
    tags.filter_map(canonical_style_tag).collect()
}

pub(super) fn is_specific_style_tag(tag: &str) -> bool {
    !matches!(tag, "abstract" | "anime" | "fantasy")
}

pub(super) fn is_content_tag(tag: &str) -> bool {
    if canonical_style_tag(tag).is_some() {
        return false;
    }
    !matches!(
        tag,
        "bright" | "dark" | "pastel" | "vibrant" | "minimal" | "landscape_orientation" | "portrait"
    )
}
