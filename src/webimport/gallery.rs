/// Supported web galleries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gallery {
    Unsplash,
    Wallhaven,
}

impl Gallery {
    pub fn slug(self) -> &'static str {
        match self {
            Gallery::Unsplash => "unsplash",
            Gallery::Wallhaven => "wallhaven",
        }
    }
}

/// Search result from a gallery.
#[derive(Debug, Clone)]
pub struct GalleryImage {
    pub id: String,
    pub url: String,
    pub width: u32,
    pub height: u32,
    pub author: Option<String>,
    pub source: Gallery,
}

impl GalleryImage {
    pub fn unsplash(
        id: impl Into<String>,
        url: impl Into<String>,
        width: u32,
        height: u32,
        author: Option<String>,
    ) -> Self {
        Self {
            id: id.into(),
            url: url.into(),
            width,
            height,
            author,
            source: Gallery::Unsplash,
        }
    }

    pub fn wallhaven(
        id: impl Into<String>,
        url: impl Into<String>,
        width: u32,
        height: u32,
    ) -> Self {
        Self {
            id: id.into(),
            url: url.into(),
            width,
            height,
            author: None,
            source: Gallery::Wallhaven,
        }
    }

    pub fn with_url(&self, url: impl Into<String>) -> Self {
        Self {
            url: url.into(),
            ..self.clone()
        }
    }

    pub fn download_extension(&self) -> &str {
        self.url
            .rsplit('/')
            .next()
            .and_then(|segment| segment.split(['?', '#']).next())
            .and_then(|segment| segment.rsplit('.').next().filter(|ext| *ext != segment))
            .filter(|ext| !ext.is_empty())
            .unwrap_or("jpg")
    }

    pub fn download_filename(&self) -> String {
        format!(
            "{}_{}.{}",
            self.source.slug(),
            self.id,
            self.download_extension()
        )
    }
}
