use anyhow::Result;
use std::path::Path;

use crate::webimport::{Gallery, GalleryImage, WebImporter};
use crate::ImportAction;

fn parse_prefixed_unsplash_id(value: &str) -> Option<&str> {
    value
        .strip_prefix("unsplash_")
        .or_else(|| value.strip_prefix("unsplash:"))
        .map(str::trim)
        .filter(|id| !id.is_empty())
}

fn unsplash_id_from_url(url: &str) -> Option<String> {
    for marker in ["/photos/", "/download/"] {
        if let Some((_, tail)) = url.split_once(marker) {
            let id = tail
                .split(['/', '?', '#'])
                .next()
                .unwrap_or("")
                .trim_matches('/');
            if !id.is_empty() {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn sanitize_filename_token(value: &str) -> String {
    let mut sanitized: String = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect();

    if sanitized.is_empty() {
        return "unsplash".to_string();
    }
    if sanitized.len() > 64 {
        sanitized.truncate(64);
    }
    sanitized
}

fn direct_unsplash_image(url: &str) -> GalleryImage {
    let tail = url
        .rsplit('/')
        .next()
        .unwrap_or("unsplash")
        .split(['?', '#'])
        .next()
        .unwrap_or("unsplash");
    let id = sanitize_filename_token(tail);
    GalleryImage {
        id,
        url: url.to_string(),
        width: 0,
        height: 0,
        author: None,
        source: Gallery::Unsplash,
    }
}

fn normalize_wallhaven_id(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches('/');
    if trimmed.is_empty() {
        return None;
    }

    let tail = trimmed.rsplit('/').next().unwrap_or(trimmed);
    let without_query = tail.split(['?', '#']).next().unwrap_or(tail);
    let without_prefix = without_query
        .strip_prefix("wallhaven-")
        .unwrap_or(without_query);
    let id = without_prefix.split('.').next().unwrap_or(without_prefix);

    if id.len() < 2 || !id.chars().all(|ch| ch.is_ascii_alphanumeric()) {
        return None;
    }

    Some(id.to_ascii_lowercase())
}

fn wallhaven_image_url(id: &str, extension: &str) -> String {
    let prefix = &id[..2];
    format!("https://w.wallhaven.cc/full/{prefix}/wallhaven-{id}.{extension}")
}

fn wallhaven_image_from_id(id: String, extension: &str) -> GalleryImage {
    GalleryImage {
        id: id.clone(),
        url: wallhaven_image_url(&id, extension),
        width: 0,
        height: 0,
        author: None,
        source: Gallery::Wallhaven,
    }
}

pub async fn cmd_import(action: ImportAction, wallpaper_dir: &Path) -> Result<()> {
    let importer = WebImporter::new();

    match action {
        ImportAction::Unsplash { query, count } => {
            if !importer.is_available(Gallery::Unsplash) {
                println!("Unsplash requires an API key.");
                println!("1. Get a free key at: https://unsplash.com/developers");
                println!("2. Set: export UNSPLASH_ACCESS_KEY=your_key");
                return Ok(());
            }

            println!("Searching Unsplash for \"{}\"...", query);
            let results = importer.search(Gallery::Unsplash, &query, 1, count).await?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nFound {} images:\n", results.len());
            for (i, img) in results.iter().enumerate() {
                let author = img.author.as_deref().unwrap_or("Unknown");
                println!(
                    "  {}. {}x{} by {} [{}]",
                    i + 1,
                    img.width,
                    img.height,
                    author,
                    img.id
                );
            }

            println!("\nDownload with: frostwall import download unsplash_<id>");
            println!(
                "  e.g.: frostwall import download unsplash_{}",
                results[0].id
            );
        }
        ImportAction::Wallhaven { query, count } => {
            println!("Searching Wallhaven for \"{}\"...", query);
            let results = importer
                .search(Gallery::Wallhaven, &query, 1, count)
                .await?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nFound {} images:\n", results.len());
            for (i, img) in results.iter().enumerate() {
                println!("  {}. {}x{} [{}]", i + 1, img.width, img.height, img.id);
            }

            println!("\nDownload with: frostwall import download <id>");
            println!("  e.g.: frostwall import download {}", results[0].id);
        }
        ImportAction::Featured { count } => {
            println!("Fetching top wallpapers from Wallhaven...");
            let results = importer.featured_wallhaven(count).await?;

            if results.is_empty() {
                println!("No results found.");
                return Ok(());
            }

            println!("\nTop {} wallpapers:\n", results.len());
            for (i, img) in results.iter().enumerate() {
                println!("  {}. {}x{} [{}]", i + 1, img.width, img.height, img.id);
            }

            println!("\nDownload with: frostwall import download <id>");
        }
        ImportAction::Download { url } => {
            // Determine source from URL/ID
            let image = if let Some(photo_id) = parse_prefixed_unsplash_id(&url) {
                importer.unsplash_photo_by_id(photo_id).await?
            } else if url.starts_with("http") {
                if url.contains("images.unsplash.com") {
                    direct_unsplash_image(&url)
                } else if url.contains("unsplash.com") {
                    let Some(photo_id) = unsplash_id_from_url(&url) else {
                        println!("Could not parse Unsplash photo ID from URL.");
                        println!("Use format: https://unsplash.com/photos/<id>");
                        return Ok(());
                    };
                    importer.unsplash_photo_by_id(&photo_id).await?
                } else if url.contains("wallhaven.cc") || url.contains("w.wallhaven") {
                    let Some(id) = normalize_wallhaven_id(&url) else {
                        println!("Invalid Wallhaven URL: {}", url);
                        return Ok(());
                    };
                    wallhaven_image_from_id(id, "jpg")
                } else {
                    println!("Unknown URL source. Supported: Unsplash, Wallhaven");
                    return Ok(());
                }
            } else {
                // Assume Wallhaven ID unless explicitly prefixed for Unsplash.
                let Some(id) = normalize_wallhaven_id(&url) else {
                    println!("Unrecognized ID format: {}", url);
                    println!("Use `unsplash_<id>` for Unsplash or a Wallhaven ID/URL.");
                    return Ok(());
                };
                wallhaven_image_from_id(id, "jpg")
            };

            println!("Downloading {}...", image.id);

            match importer.download(&image, wallpaper_dir).await {
                Ok(path) => {
                    println!("Downloaded to: {}", path.display());
                    println!("\nRun 'frostwall scan' to add it to the cache.");
                }
                Err(e) => {
                    // Try alternative URL formats for Wallhaven
                    if image.source == Gallery::Wallhaven {
                        // Try PNG format
                        let png_image = GalleryImage {
                            url: wallhaven_image_url(&image.id, "png"),
                            ..image.clone()
                        };
                        if let Ok(path) = importer.download(&png_image, wallpaper_dir).await {
                            println!("Downloaded to: {}", path.display());
                            println!("\nRun 'frostwall scan' to add it to the cache.");
                            return Ok(());
                        }
                    }
                    println!("Download failed: {}", e);
                    println!("The image might not exist or the URL format has changed.");
                }
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_prefixed_unsplash_id_supports_both_prefixes() {
        assert_eq!(
            parse_prefixed_unsplash_id("unsplash_abc123"),
            Some("abc123")
        );
        assert_eq!(parse_prefixed_unsplash_id("unsplash:xyz"), Some("xyz"));
        assert_eq!(parse_prefixed_unsplash_id("abc123"), None);
    }

    #[test]
    fn unsplash_id_from_url_parses_photo_urls() {
        assert_eq!(
            unsplash_id_from_url("https://unsplash.com/photos/AbCdEf12345"),
            Some("AbCdEf12345".to_string())
        );
        assert_eq!(
            unsplash_id_from_url("https://unsplash.com/photos/AbCdEf12345/download?force=true"),
            Some("AbCdEf12345".to_string())
        );
    }

    #[test]
    fn normalize_wallhaven_id_handles_id_and_url() {
        assert_eq!(normalize_wallhaven_id("w8x7y9"), Some("w8x7y9".to_string()));
        assert_eq!(
            normalize_wallhaven_id("https://wallhaven.cc/w/w8x7y9"),
            Some("w8x7y9".to_string())
        );
        assert_eq!(
            normalize_wallhaven_id("https://w.wallhaven.cc/full/w8/wallhaven-w8x7y9.jpg"),
            Some("w8x7y9".to_string())
        );
    }

    #[test]
    fn normalize_wallhaven_id_rejects_invalid_values() {
        assert_eq!(normalize_wallhaven_id(""), None);
        assert_eq!(normalize_wallhaven_id("åäö"), None);
        assert_eq!(normalize_wallhaven_id("x"), None);
    }
}
