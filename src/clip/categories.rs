//! Category embedding catalog: base CLIP categories plus library-specific
//! mixes blended from those bases without re-running text encoding.

use crate::clip_embeddings_bin::{category_embeddings, EMBEDDING_DIM};

/// Extra categories tuned for this wallpaper library.
/// Each is a weighted blend of base CLIP embeddings, so we get new tag types
/// without regenerating text embeddings from a transformer.
pub(super) const LIBRARY_CATEGORY_MIXES: &[(&str, &[(&str, f32)])] = &[
    (
        "pixel_art",
        &[
            ("retro", 0.35),
            ("vibrant", 0.25),
            ("minimal", 0.20),
            ("geometric", 0.20),
        ],
    ),
    (
        "anime_character",
        &[
            ("anime", 0.50),
            ("portrait", 0.25),
            ("illustration", 0.15),
            ("vibrant", 0.10),
        ],
    ),
    (
        "fantasy_landscape",
        &[
            ("fantasy", 0.40),
            ("nature", 0.25),
            ("mountain", 0.15),
            ("dramatic", 0.10),
            ("landscape_orientation", 0.10),
        ],
    ),
    (
        "epic_battle",
        &[
            ("fantasy", 0.30),
            ("dramatic", 0.30),
            ("dark", 0.15),
            ("samurai", 0.15),
            ("vibrant", 0.10),
        ],
    ),
    (
        "sakura",
        &[
            ("flowers", 0.30),
            ("anime", 0.25),
            ("pastel", 0.25),
            ("serene", 0.20),
        ],
    ),
    (
        "nightscape",
        &[
            ("dark", 0.35),
            ("space", 0.25),
            ("city", 0.20),
            ("neon", 0.20),
        ],
    ),
    (
        "painterly",
        &[
            ("oil_painting", 0.35),
            ("watercolor", 0.25),
            ("fantasy", 0.20),
            ("nature", 0.10),
            ("vintage", 0.10),
        ],
    ),
    (
        "concept_art",
        &[
            ("digital_art", 0.35),
            ("fantasy", 0.25),
            ("dramatic", 0.20),
            ("illustration", 0.20),
        ],
    ),
    (
        "ethereal",
        &[
            ("pastel", 0.30),
            ("serene", 0.25),
            ("fantasy", 0.25),
            ("bright", 0.20),
        ],
    ),
    (
        "moody_fantasy",
        &[
            ("dark", 0.30),
            ("fantasy", 0.30),
            ("gothic", 0.20),
            ("forest", 0.10),
            ("mountain", 0.10),
        ],
    ),
];

fn find_base_embedding(name: &str) -> Option<&'static [f32; EMBEDDING_DIM]> {
    category_embeddings()
        .iter()
        .find(|(base_name, _)| base_name == name)
        .map(|(_, embedding)| embedding)
}

pub(super) fn normalize_embedding(mut embedding: Vec<f32>) -> Vec<f32> {
    let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut embedding {
            *value /= norm;
        }
    }
    embedding
}

fn build_mixed_embedding(parts: &[(&str, f32)]) -> Option<Vec<f32>> {
    let mut mixed = vec![0.0f32; EMBEDDING_DIM];
    let mut total_weight = 0.0f32;

    for (base_name, weight) in parts {
        let Some(base) = find_base_embedding(base_name) else {
            continue;
        };
        for (idx, value) in base.iter().enumerate() {
            mixed[idx] += *value * *weight;
        }
        total_weight += *weight;
    }

    if total_weight <= 0.0 {
        return None;
    }

    for value in &mut mixed {
        *value /= total_weight;
    }
    Some(normalize_embedding(mixed))
}

/// Combine the base CLIP categories with the mixed library-specific categories
/// into a single (name, embedding) catalog used for cosine-similarity scoring.
pub(super) fn build_category_embeddings() -> Vec<(String, Vec<f32>)> {
    let mut categories: Vec<(String, Vec<f32>)> = category_embeddings()
        .iter()
        .map(|(name, embedding)| (name.clone(), embedding.to_vec()))
        .collect();

    for (name, parts) in LIBRARY_CATEGORY_MIXES {
        if let Some(embedding) = build_mixed_embedding(parts) {
            categories.push((name.to_string(), embedding));
        }
    }

    categories
}
