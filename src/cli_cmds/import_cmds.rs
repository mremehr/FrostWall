use anyhow::Result;
use std::path::Path;

use crate::ImportAction;

pub fn cmd_import(action: ImportAction, wallpaper_dir: &Path) -> Result<()> {
    use crate::webimport::{Gallery, WebImporter};

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
            let results = importer.search(Gallery::Unsplash, &query, 1, count)?;

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

            println!("\nDownload with: frostwall import download <id>");
            println!("Or download all with: frostwall import download unsplash_<id>");
        }
        ImportAction::Wallhaven { query, count } => {
            println!("Searching Wallhaven for \"{}\"...", query);
            let results = importer.search(Gallery::Wallhaven, &query, 1, count)?;

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
            let results = importer.featured_wallhaven(count)?;

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
            let image = if url.starts_with("http") {
                // Full URL - try to determine source
                if url.contains("unsplash.com") {
                    println!("Direct Unsplash URLs require the search command first.");
                    return Ok(());
                } else if url.contains("wallhaven.cc") || url.contains("w.wallhaven") {
                    // Extract ID from Wallhaven URL
                    let id = url.rsplit('/').next().unwrap_or(&url);
                    let id = id.split('.').next().unwrap_or(id);
                    crate::webimport::GalleryImage {
                        id: id.to_string(),
                        url: format!(
                            "https://w.wallhaven.cc/full/{}/wallhaven-{}.jpg",
                            &id[..2.min(id.len())],
                            id
                        ),
                        width: 0,
                        height: 0,
                        author: None,
                        source: Gallery::Wallhaven,
                    }
                } else {
                    println!("Unknown URL source. Supported: Unsplash, Wallhaven");
                    return Ok(());
                }
            } else {
                // Assume Wallhaven ID
                let full_url = format!(
                    "https://w.wallhaven.cc/full/{}/wallhaven-{}.jpg",
                    &url[..2.min(url.len())],
                    url
                );
                crate::webimport::GalleryImage {
                    id: url.clone(),
                    url: full_url,
                    width: 0,
                    height: 0,
                    author: None,
                    source: Gallery::Wallhaven,
                }
            };

            println!("Downloading {}...", image.id);

            match importer.download(&image, wallpaper_dir) {
                Ok(path) => {
                    println!("Downloaded to: {}", path.display());
                    println!("\nRun 'frostwall scan' to add it to the cache.");
                }
                Err(e) => {
                    // Try alternative URL formats for Wallhaven
                    if image.source == Gallery::Wallhaven {
                        // Try PNG format
                        let png_url = image.url.replace(".jpg", ".png");
                        let png_image = crate::webimport::GalleryImage {
                            url: png_url,
                            ..image.clone()
                        };
                        if let Ok(path) = importer.download(&png_image, wallpaper_dir) {
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
