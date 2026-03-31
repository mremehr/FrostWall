//! Web gallery import for wallpapers.
//!
//! Download wallpapers from popular galleries like Unsplash and Wallhaven.

mod api;
mod download;
mod encoding;
mod gallery;

#[cfg(test)]
mod tests;

pub use api::WebImporter;
pub use gallery::{Gallery, GalleryImage};
