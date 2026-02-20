use anyhow::Result;
use std::path::Path;

use crate::wallpaper;

pub async fn cmd_auto_tag(
    wallpaper_dir: &Path,
    incremental: bool,
    threshold: f32,
    max_tags: usize,
    verbose: bool,
) -> Result<()> {
    use crate::clip::ClipTagger;

    println!("Initializing CLIP model...");

    let mut tagger = ClipTagger::new().await?;

    let mut cache = wallpaper::WallpaperCache::load_or_scan_for_ai(wallpaper_dir)?;

    let to_process: Vec<usize> = cache
        .wallpapers
        .iter()
        .enumerate()
        .filter(|(_, wp)| !incremental || wp.auto_tags.is_empty() || wp.embedding.is_none())
        .map(|(i, _)| i)
        .collect();

    if to_process.is_empty() {
        println!("All wallpapers already tagged.");
        return Ok(());
    }

    println!("Auto-tagging {} wallpapers...", to_process.len());

    for (progress, idx) in to_process.iter().enumerate() {
        let wp = &cache.wallpapers[*idx];
        let path = wp.path.clone();

        // Show verbose debug output only for first image
        let show_debug = verbose && progress == 0;
        if show_debug {
            eprintln!("\n=== Debug output for first image ===");
            eprintln!("Image: {}", path.display());
        }

        match tagger.analyze_image_verbose(&path, threshold, show_debug) {
            Ok(mut analysis) => {
                // Limit to max_tags (tags are already sorted by confidence)
                if max_tags > 0 && analysis.tags.len() > max_tags {
                    analysis.tags.truncate(max_tags);
                }

                if verbose {
                    let tag_names: Vec<_> = analysis.tags.iter().map(|t| &t.name).collect();
                    println!(
                        "[{}/{}] {}: {:?} (emb={})",
                        progress + 1,
                        to_process.len(),
                        path.file_name().unwrap_or_default().to_string_lossy(),
                        tag_names,
                        analysis.embedding.len(),
                    );
                } else if (progress + 1) % 10 == 0 || progress + 1 == to_process.len() {
                    eprint!("\rProgress: {}/{}", progress + 1, to_process.len());
                }

                cache.wallpapers[*idx].set_auto_tags(analysis.tags);
                cache.wallpapers[*idx].set_embedding(analysis.embedding);
            }
            Err(e) => {
                eprintln!("\nWarning: Failed to tag {}: {}", path.display(), e);
            }
        }
    }

    if !verbose {
        eprintln!(); // Newline after progress
    }

    cache.save()?;

    // Show summary
    let tags = crate::clip::ClipTagger::available_tags();
    println!("\nTag distribution:");
    for tag in tags {
        let count = cache
            .wallpapers
            .iter()
            .filter(|wp| wp.auto_tags.iter().any(|t| t.name == tag))
            .count();
        if count > 0 {
            println!("  {}: {}", tag, count);
        }
    }

    println!("\nDone! Tags saved to cache.");
    Ok(())
}
