//! Web gallery import for wallpapers
//!
//! Download wallpapers from popular galleries like Unsplash and Wallhaven.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// Supported web galleries
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gallery {
    Unsplash,
    Wallhaven,
}

impl Gallery {
    pub fn name(&self) -> &'static str {
        match self {
            Gallery::Unsplash => "Unsplash",
            Gallery::Wallhaven => "Wallhaven",
        }
    }
}

/// Search result from a gallery
#[derive(Debug, Clone)]
pub struct GalleryImage {
    pub id: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub author: Option<String>,
    pub source: Gallery,
}

// Unsplash API response structures
#[derive(Debug, Deserialize)]
struct UnsplashPhoto {
    id: String,
    width: u32,
    height: u32,
    urls: UnsplashUrls,
    user: UnsplashUser,
}

#[derive(Debug, Deserialize)]
struct UnsplashUrls {
    raw: String,
}

#[derive(Debug, Deserialize)]
struct UnsplashUser {
    name: String,
}

#[derive(Debug, Deserialize)]
struct UnsplashSearchResponse {
    results: Vec<UnsplashPhoto>,
}

// Wallhaven API response structures
#[derive(Debug, Deserialize)]
struct WallhavenResponse {
    data: Vec<WallhavenImage>,
}

#[derive(Debug, Deserialize)]
struct WallhavenImage {
    id: String,
    path: String,
    dimension_x: u32,
    dimension_y: u32,
}

/// Web import client
pub struct WebImporter {
    client: reqwest::blocking::Client,
    unsplash_key: Option<String>,
    wallhaven_key: Option<String>,
}

impl WebImporter {
    /// Create a new web importer
    pub fn new() -> Self {
        Self {
            client: reqwest::blocking::Client::builder()
                .user_agent(format!("FrostWall/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_else(|_| reqwest::blocking::Client::new()),
            unsplash_key: std::env::var("UNSPLASH_ACCESS_KEY").ok(),
            wallhaven_key: std::env::var("WALLHAVEN_API_KEY").ok(),
        }
    }

    /// Check if a gallery is available (has API key if required)
    pub fn is_available(&self, gallery: Gallery) -> bool {
        match gallery {
            Gallery::Unsplash => self.unsplash_key.is_some(),
            Gallery::Wallhaven => true, // Public API available without key
        }
    }

    /// Search for images in a gallery
    pub fn search(
        &self,
        gallery: Gallery,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GalleryImage>> {
        match gallery {
            Gallery::Unsplash => self.search_unsplash(query, page, per_page),
            Gallery::Wallhaven => self.search_wallhaven(query, page, per_page),
        }
    }

    fn require_unsplash_key(&self) -> Result<&str> {
        self.unsplash_key
            .as_deref()
            .context("Unsplash API key required. Set UNSPLASH_ACCESS_KEY environment variable.")
    }

    /// Search Unsplash
    fn search_unsplash(&self, query: &str, page: u32, per_page: u32) -> Result<Vec<GalleryImage>> {
        let api_key = self.require_unsplash_key()?;

        let url = format!(
            "https://api.unsplash.com/search/photos?query={}&page={}&per_page={}&orientation=landscape",
            urlencoding::encode(query),
            page,
            per_page.min(30) // Unsplash max is 30
        );

        let response: UnsplashSearchResponse = self
            .client
            .get(&url)
            .header("Authorization", format!("Client-ID {}", api_key))
            .send()
            .context("Failed to connect to Unsplash")?
            .json()
            .context("Failed to parse Unsplash response")?;

        Ok(response
            .results
            .into_iter()
            .map(|photo| GalleryImage {
                id: photo.id,
                url: format!("{}?w=3840&q=85", photo.urls.raw), // 4K quality
                width: photo.width,
                height: photo.height,
                author: Some(photo.user.name),
                source: Gallery::Unsplash,
            })
            .collect())
    }

    /// Fetch a single Unsplash image by photo ID.
    pub fn unsplash_photo_by_id(&self, photo_id: &str) -> Result<GalleryImage> {
        let api_key = self.require_unsplash_key()?;
        let url = format!(
            "https://api.unsplash.com/photos/{}",
            urlencoding::encode(photo_id)
        );

        let photo: UnsplashPhoto = self
            .client
            .get(&url)
            .header("Authorization", format!("Client-ID {}", api_key))
            .send()
            .context("Failed to connect to Unsplash")?
            .json()
            .context("Failed to parse Unsplash response")?;

        Ok(GalleryImage {
            id: photo.id,
            url: format!("{}?w=3840&q=85", photo.urls.raw),
            width: photo.width,
            height: photo.height,
            author: Some(photo.user.name),
            source: Gallery::Unsplash,
        })
    }

    /// Search Wallhaven
    fn search_wallhaven(&self, query: &str, page: u32, per_page: u32) -> Result<Vec<GalleryImage>> {
        let mut url = format!(
            "https://wallhaven.cc/api/v1/search?q={}&page={}&categories=111&purity=100&sorting=relevance&order=desc",
            urlencoding::encode(query),
            page,
        );

        // Add API key if available (allows access to NSFW if enabled in account)
        if let Some(key) = &self.wallhaven_key {
            url.push_str(&format!("&apikey={}", urlencoding::encode(key)));
        }

        let response: WallhavenResponse = self
            .client
            .get(&url)
            .send()
            .context("Failed to connect to Wallhaven")?
            .json()
            .context("Failed to parse Wallhaven response")?;

        Ok(response
            .data
            .into_iter()
            .take(per_page as usize)
            .map(|img| GalleryImage {
                id: img.id,
                url: img.path,
                width: img.dimension_x,
                height: img.dimension_y,
                author: None,
                source: Gallery::Wallhaven,
            })
            .collect())
    }

    /// Download an image to the specified directory
    pub fn download(&self, image: &GalleryImage, dest_dir: &Path) -> Result<PathBuf> {
        // Create filename from ID and extension
        let extension = image
            .url
            .rsplit('.')
            .next()
            .and_then(|ext| ext.split('?').next())
            .unwrap_or("jpg");

        let filename = format!(
            "{}_{}.{}",
            image.source.name().to_lowercase(),
            image.id,
            extension
        );
        let dest_path = dest_dir.join(&filename);

        // Skip if already exists
        if dest_path.exists() {
            return Ok(dest_path);
        }

        // Download the image
        let response = self
            .client
            .get(&image.url)
            .send()
            .context("Failed to download image")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        let bytes = response.bytes().context("Failed to read image data")?;

        // Ensure directory exists
        std::fs::create_dir_all(dest_dir)?;

        // Write to file
        std::fs::write(&dest_path, &bytes).context("Failed to save image")?;

        Ok(dest_path)
    }

    /// Get random featured wallpapers from Wallhaven
    pub fn featured_wallhaven(&self, count: u32) -> Result<Vec<GalleryImage>> {
        let url = "https://wallhaven.cc/api/v1/search?sorting=toplist&topRange=1M&categories=111&purity=100&atleast=1920x1080";

        let response: WallhavenResponse = self
            .client
            .get(url)
            .send()
            .context("Failed to connect to Wallhaven")?
            .json()
            .context("Failed to parse Wallhaven response")?;

        Ok(response
            .data
            .into_iter()
            .take(count as usize)
            .map(|img| GalleryImage {
                id: img.id,
                url: img.path,
                width: img.dimension_x,
                height: img.dimension_y,
                author: None,
                source: Gallery::Wallhaven,
            })
            .collect())
    }
}

impl Default for WebImporter {
    fn default() -> Self {
        Self::new()
    }
}

/// URL encoding helper
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for c in s.chars() {
            match c {
                'a'..='z' | 'A'..='Z' | '0'..='9' | '-' | '_' | '.' | '~' => {
                    result.push(c);
                }
                ' ' => result.push_str("%20"),
                _ => {
                    for byte in c.to_string().as_bytes() {
                        result.push_str(&format!("%{:02X}", byte));
                    }
                }
            }
        }
        result
    }
}
