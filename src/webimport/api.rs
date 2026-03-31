use super::encoding;
use super::gallery::{Gallery, GalleryImage};
use anyhow::{Context, Result};
use serde::Deserialize;

// Unsplash API response structures.
#[derive(Debug, Deserialize)]
struct UnsplashPhoto {
    id: String,
    width: u32,
    height: u32,
    urls: UnsplashUrls,
    user: UnsplashUser,
}

impl UnsplashPhoto {
    fn into_gallery_image(self) -> GalleryImage {
        GalleryImage::unsplash(
            self.id,
            format!("{}?w=3840&q=85", self.urls.raw),
            self.width,
            self.height,
            Some(self.user.name),
        )
    }
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

// Wallhaven API response structures.
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

impl WallhavenImage {
    fn into_gallery_image(self) -> GalleryImage {
        GalleryImage::wallhaven(self.id, self.path, self.dimension_x, self.dimension_y)
    }
}

/// Web import client.
pub struct WebImporter {
    pub(super) client: reqwest::Client,
    unsplash_key: Option<String>,
    wallhaven_key: Option<String>,
}

impl WebImporter {
    /// Create a new web importer.
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

    /// Check if a gallery is available (has API key if required).
    pub fn is_available(&self, gallery: Gallery) -> bool {
        match gallery {
            Gallery::Unsplash => self.unsplash_key.is_some(),
            Gallery::Wallhaven => true,
        }
    }

    /// Search for images in a gallery.
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

    async fn search_unsplash(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GalleryImage>> {
        let api_key = self.require_unsplash_key()?;
        let url = format!(
            "https://api.unsplash.com/search/photos?query={}&page={}&per_page={}&orientation=landscape",
            encoding::encode(query),
            page,
            per_page.min(30)
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
            .map(UnsplashPhoto::into_gallery_image)
            .collect())
    }

    /// Fetch a single Unsplash image by photo ID.
    pub async fn unsplash_photo_by_id(&self, photo_id: &str) -> Result<GalleryImage> {
        let api_key = self.require_unsplash_key()?;
        let url = format!(
            "https://api.unsplash.com/photos/{}",
            encoding::encode(photo_id)
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

        Ok(photo.into_gallery_image())
    }

    async fn search_wallhaven(
        &self,
        query: &str,
        page: u32,
        per_page: u32,
    ) -> Result<Vec<GalleryImage>> {
        let mut url = format!(
            "https://wallhaven.cc/api/v1/search?q={}&page={}&categories=111&purity=100&sorting=relevance&order=desc",
            encoding::encode(query),
            page,
        );

        if let Some(key) = &self.wallhaven_key {
            url.push_str(&format!("&apikey={}", encoding::encode(key)));
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
            .map(WallhavenImage::into_gallery_image)
            .collect())
    }

    /// Get random featured wallpapers from Wallhaven.
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
            .map(WallhavenImage::into_gallery_image)
            .collect())
    }
}

impl Default for WebImporter {
    fn default() -> Self {
        Self::new()
    }
}
