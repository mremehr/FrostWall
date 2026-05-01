use anyhow::Result;
use std::path::Path;

use super::support::{load_cache_with_config, load_config};
use crate::{utils, wallpaper};

pub async fn cmd_auto_tag(
    wallpaper_dir: &Path,
    incremental: bool,
    threshold: f32,
    max_tags: usize,
    verbose: bool,
) -> Result<()> {
    use crate::clip::ClipTagger;

    println!("Initializing CLIP model...");

    let config = load_config()?;
    let mut tagger = ClipTagger::new(&config).await?;
    let mut cache = load_cache_with_config(
        wallpaper_dir,
        &config,
        wallpaper::CacheLoadMode::MetadataOnly,
    )?;

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

    let batch_size = tagger.batch_size();
    let backend = if tagger.is_gpu_accelerated() {
        "CUDA"
    } else {
        "CPU"
    };
    println!(
        "Auto-tagging {} wallpapers in batches of {} ({backend})...",
        to_process.len(),
        batch_size,
    );

    for (batch_index, chunk) in to_process.chunks(batch_size).enumerate() {
        let batch_paths: Vec<_> = chunk
            .iter()
            .map(|idx| cache.wallpapers[*idx].path.clone())
            .collect();
        let show_debug = verbose && batch_index == 0;
        if show_debug {
            eprintln!("\n=== Debug output for first image ===");
            if let Some(path) = batch_paths.first() {
                eprintln!("Image: {}", path.display());
            }
        }

        for (offset, (idx, result)) in chunk
            .iter()
            .copied()
            .zip(tagger.analyze_images_batch_verbose(&batch_paths, threshold, show_debug))
            .enumerate()
        {
            let progress = batch_index * batch_size + offset;
            let path = batch_paths[offset].clone();

            match result {
                Ok(mut analysis) => {
                    if max_tags > 0 && analysis.tags.len() > max_tags {
                        analysis.tags.truncate(max_tags);
                    }

                    if verbose {
                        let tag_names: Vec<_> = analysis.tags.iter().map(|t| &t.name).collect();
                        println!(
                            "[{}/{}] {}: {:?} (emb={})",
                            progress + 1,
                            to_process.len(),
                            utils::display_path_name(&path),
                            tag_names,
                            analysis.embedding.len(),
                        );
                    } else if (progress + 1) % 10 == 0 || progress + 1 == to_process.len() {
                        eprint!("\rProgress: {}/{}", progress + 1, to_process.len());
                    }

                    cache.wallpapers[idx].set_auto_tags(analysis.tags);
                    cache.wallpapers[idx].set_embedding(analysis.embedding);
                }
                Err(e) => {
                    eprintln!("\nWarning: Failed to tag {}: {}", path.display(), e);
                }
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
