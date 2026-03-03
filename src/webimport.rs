//! Web gallery import for wallpapers
//!
//! Download wallpapers from popular galleries like Unsplash and Wallhaven.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;

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
    client: reqwest::Client,
    unsplash_key: Option<String>,
    wallhaven_key: Option<String>,
}

impl WebImporter {
    /// Create a new web importer
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::builder()
                .user_agent(format!("FrostWall/{}", env!("CARGO_PKG_VERSION")))
                .build()
                .unwrap_or_else(|_| reqwest::Client::new()),
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
    pub async fn search(
        &self,
        gallery: Gallery,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GalleryImage>> {
        match gallery {
            Gallery::Unsplash => self.search_unsplash(query, page, per_page).await,
            Gallery::Wallhaven => self.search_wallhaven(query, page, per_page).await,
        }
    }

    fn require_unsplash_key(&self) -> Result<&str> {
        self.unsplash_key
            .as_deref()
            .context("Unsplash API key required. Set UNSPLASH_ACCESS_KEY environment variable.")
    }

    /// Search Unsplash
    async fn search_unsplash(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GalleryImage>> {
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
            .await
            .context("Failed to connect to Unsplash")?
            .json()
            .await
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
    pub async fn unsplash_photo_by_id(&self, photo_id: &str) -> Result<GalleryImage> {
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
            .await
            .context("Failed to connect to Unsplash")?
            .json()
            .await
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
    async fn search_wallhaven(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GalleryImage>> {
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
            .await
            .context("Failed to connect to Wallhaven")?
            .json()
            .await
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
    pub async fn download(&self, image: &GalleryImage, dest_dir: &Path) -> Result<PathBuf> {
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
            .await
            .context("Failed to download image")?;

        if !response.status().is_success() {
            anyhow::bail!("Download failed with status: {}", response.status());
        }

        if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
            let is_image = content_type
                .to_str()
                .map(|v| v.starts_with("image/"))
                .unwrap_or(false);
            if !is_image {
                let content_type_str = String::from_utf8_lossy(content_type.as_bytes());
                anyhow::bail!("Unexpected content type: {}", content_type_str);
            }
        }

        if let Some(content_len) = response.content_length() {
            if content_len > MAX_DOWNLOAD_BYTES {
                anyhow::bail!(
                    "Image too large: {} bytes (max {})",
                    content_len,
                    MAX_DOWNLOAD_BYTES
                );
            }
        }

        // Ensure directory exists
        fs::create_dir_all(dest_dir).await?;

        let tmp_path = dest_path.with_extension(format!("{extension}.part"));
        let mut file = fs::File::create(&tmp_path)
            .await
            .with_context(|| format!("Failed to create temp file: {}", tmp_path.display()))?;

        // Stream download to disk to avoid buffering large files in memory.
        let mut response = response;
        let mut written: u64 = 0;
        while let Some(chunk) = response
            .chunk()
            .await
            .context("Failed to read image data")?
        {
            written += chunk.len() as u64;
            if written > MAX_DOWNLOAD_BYTES {
                let _ = fs::remove_file(&tmp_path).await;
                anyhow::bail!(
                    "Image exceeded max download size ({} bytes)",
                    MAX_DOWNLOAD_BYTES
                );
            }

            file.write_all(&chunk)
                .await
                .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
        }

        file.flush()
            .await
            .with_context(|| format!("Failed to flush {}", tmp_path.display()))?;

        // Validate that the downloaded file is a readable image before finalizing.
        image::image_dimensions(&tmp_path).with_context(|| {
            format!(
                "Downloaded file is not a valid image: {}",
                tmp_path.display()
            )
        })?;

        fs::rename(&tmp_path, &dest_path)
            .await
            .with_context(|| format!("Failed to finalize image file: {}", dest_path.display()))?;

        Ok(dest_path)
    }

    /// Get random featured wallpapers from Wallhaven
    pub async fn featured_wallhaven(&self, count: u32) -> Result<Vec<GalleryImage>> {
        let url = "https://wallhaven.cc/api/v1/search?sorting=toplist&topRange=1M&categories=111&purity=100&atleast=1920x1080";

        let response: WallhavenResponse = self
            .client
            .get(url)
            .send()
            .await
            .context("Failed to connect to Wallhaven")?
            .json()
            .await
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
