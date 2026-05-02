//! CLIP image preprocessing (resize + normalize) and ONNX output-tensor parsing.

use anyhow::{ensure, Context, Result};
use std::path::Path;

use crate::thumbnail::ThumbnailLookup;

/// CLIP image input size (ViT-B/32).
pub const CLIP_IMAGE_SIZE: u32 = 224;
pub(super) const CLIP_CHANNELS: usize = 3;
pub(super) const CLIP_SAMPLE_LEN: usize =
    CLIP_CHANNELS * CLIP_IMAGE_SIZE as usize * CLIP_IMAGE_SIZE as usize;

const MEAN: [f32; 3] = [0.481_454_66, 0.457_827_5, 0.408_210_73];
const STD: [f32; 3] = [0.268_629_54, 0.261_302_6, 0.275_777_1];

/// Load and preprocess an image for CLIP visual inference.
///
/// Reuses an on-disk thumbnail when available (faster decode) and falls back
/// to the source path. Output is the normalized CHW float tensor ready for
/// batching.
pub(super) fn preprocess_image_with_lookup(
    thumbnail_lookup: &ThumbnailLookup,
    path: &Path,
) -> Result<Vec<f32>> {
    let img = if let Some(thumb_path) = thumbnail_lookup.find(path) {
        image::open(&thumb_path)
            .or_else(|_| image::open(path))
            .context("Failed to open image")?
    } else {
        image::open(path).context("Failed to open image")?
    };

    let img = img.resize_exact(
        CLIP_IMAGE_SIZE,
        CLIP_IMAGE_SIZE,
        image::imageops::FilterType::Triangle,
    );
    let rgb = img.to_rgb8();

    let mut data = Vec::with_capacity(CLIP_SAMPLE_LEN);
    for channel in 0..CLIP_CHANNELS {
        for y in 0..CLIP_IMAGE_SIZE {
            for x in 0..CLIP_IMAGE_SIZE {
                let pixel = rgb.get_pixel(x, y);
                let value = (pixel[channel] as f32 / 255.0 - MEAN[channel]) / STD[channel];
                data.push(value);
            }
        }
    }

    Ok(data)
}

/// Convert ONNX output tensor data into per-image embeddings.
///
/// Handles three CLIP output shapes:
/// - `[batch, seq, hidden]` — token output, take CLS token (first per item)
/// - `[batch, hidden]`      — already pooled, take whole row
/// - `[hidden]`             — single image, no batch dim
pub(super) fn extract_batch_embeddings(
    shape: &[usize],
    embedding_data: &[f32],
    batch_size: usize,
    verbose: bool,
) -> Result<Vec<Vec<f32>>> {
    match shape {
        [reported_batch, seq_len, hidden_dim] => {
            ensure!(
                *reported_batch == batch_size,
                "Model returned batch {} for {} inputs",
                reported_batch,
                batch_size
            );
            let per_image = seq_len * hidden_dim;
            ensure!(
                embedding_data.len() == batch_size * per_image,
                "Unexpected 3D output length {} for shape {:?}",
                embedding_data.len(),
                shape
            );
            if verbose {
                eprintln!(
                    "  3D tensor, taking first {} values per batch item (CLS token)",
                    hidden_dim
                );
            }
            Ok((0..batch_size)
                .map(|batch_index| {
                    let start = batch_index * per_image;
                    embedding_data[start..start + hidden_dim].to_vec()
                })
                .collect())
        }
        [reported_batch, hidden_dim] => {
            ensure!(
                *reported_batch == batch_size,
                "Model returned batch {} for {} inputs",
                reported_batch,
                batch_size
            );
            ensure!(
                embedding_data.len() == batch_size * hidden_dim,
                "Unexpected 2D output length {} for shape {:?}",
                embedding_data.len(),
                shape
            );
            if verbose {
                eprintln!("  2D tensor, taking {} values per batch item", hidden_dim);
            }
            Ok((0..batch_size)
                .map(|batch_index| {
                    let start = batch_index * hidden_dim;
                    embedding_data[start..start + hidden_dim].to_vec()
                })
                .collect())
        }
        [_] if batch_size == 1 => {
            if verbose {
                eprintln!("  Using all {} values", embedding_data.len());
            }
            Ok(vec![embedding_data.to_vec()])
        }
        _ => anyhow::bail!(
            "Unsupported CLIP output shape {:?} for batch size {}",
            shape,
            batch_size
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::extract_batch_embeddings;

    #[test]
    fn extract_batch_embeddings_reads_cls_token_per_item() {
        let embeddings = extract_batch_embeddings(
            &[2, 3, 4],
            &[
                1.0, 2.0, 3.0, 4.0, 40.0, 41.0, 42.0, 43.0, 80.0, 81.0, 82.0, 83.0, 5.0, 6.0, 7.0,
                8.0, 50.0, 51.0, 52.0, 53.0, 90.0, 91.0, 92.0, 93.0,
            ],
            2,
            false,
        )
        .expect("extract embeddings");

        assert_eq!(
            embeddings,
            vec![vec![1.0, 2.0, 3.0, 4.0], vec![5.0, 6.0, 7.0, 8.0]]
        );
    }

    #[test]
    fn extract_batch_embeddings_reads_pooled_2d_output() {
        let embeddings =
            extract_batch_embeddings(&[2, 3], &[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], 2, false)
                .expect("extract embeddings");

        assert_eq!(embeddings, vec![vec![1.0, 2.0, 3.0], vec![4.0, 5.0, 6.0]]);
    }
}
