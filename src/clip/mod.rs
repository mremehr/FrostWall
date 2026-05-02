//! CLIP-based auto-tagging for wallpapers.
//!
//! Uses ONNX Runtime with the CLIP ViT-B/32 visual encoder to tag images
//! with semantic categories like "nature", "city", "space", etc.
//!
//! Text embeddings are pre-computed and stored as a compact binary file
//! (`data/embeddings.bin`) embedded at compile time via
//! [`crate::clip_embeddings_bin`]. This module compiles unconditionally so
//! [`AutoTag`] can be (de)serialized from the wallpaper cache without the
//! `clip` feature; the inference machinery is gated behind it.

#[cfg(feature = "clip")]
mod categories;
#[cfg(feature = "clip")]
mod model;
#[cfg(feature = "clip")]
mod preprocess;
#[cfg(feature = "clip")]
mod tagger;

#[cfg(feature = "clip")]
pub use tagger::ClipTagger;

/// Auto-generated tag with confidence score.
///
/// Always available so the wallpaper cache can persist tags regardless of
/// whether the `clip` feature is built.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoTag {
    pub name: String,
    pub confidence: f32,
}
