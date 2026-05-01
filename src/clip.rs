//! CLIP-based auto-tagging for wallpapers
//!
//! Uses ONNX Runtime with CLIP ViT-B/32 visual encoder to automatically tag images
//! with semantic categories like "nature", "city", "space", etc.
//!
//! The text embeddings are pre-computed and stored as a compact binary file
//! (data/embeddings.bin) loaded at compile time via clip_embeddings_bin.rs.

#[cfg(feature = "clip")]
use anyhow::{ensure, Context, Result};
#[cfg(feature = "clip")]
use futures_util::StreamExt;
#[cfg(feature = "clip")]
use indicatif::{ProgressBar, ProgressStyle};
#[cfg(feature = "clip")]
use ort::session::Session;
#[cfg(feature = "clip")]
use rayon::prelude::*;
#[cfg(feature = "clip")]
use sha2::{Digest, Sha256};
#[cfg(feature = "clip")]
use std::io::Write;
#[cfg(feature = "clip")]
use std::path::{Path, PathBuf};

#[cfg(feature = "clip")]
use crate::app::Config;
#[cfg(feature = "clip")]
use crate::clip_embeddings_bin::{category_embeddings, EMBEDDING_DIM};
#[cfg(feature = "clip")]
use crate::thumbnail::ThumbnailLookup;
#[cfg(feature = "clip")]
use crate::utils::project_cache_dir;

/// CLIP image input size (ViT-B/32)
#[cfg(feature = "clip")]
pub const CLIP_IMAGE_SIZE: u32 = 224;
#[cfg(feature = "clip")]
const CLIP_CHANNELS: usize = 3;
#[cfg(feature = "clip")]
const CLIP_SAMPLE_LEN: usize = CLIP_CHANNELS * CLIP_IMAGE_SIZE as usize * CLIP_IMAGE_SIZE as usize;

/// Auto-generated tag with confidence score
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AutoTag {
    pub name: String,
    pub confidence: f32,
}

/// Default model URL from HuggingFace.
/// Uses Qdrant's model which outputs proper 512-dim projected embeddings.
#[cfg(feature = "clip")]
const DEFAULT_VISUAL_MODEL_URL: &str =
    "https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx";

/// SHA256 checksum for the default visual model (Qdrant/clip-ViT-B-32-vision).
#[cfg(feature = "clip")]
const DEFAULT_VISUAL_MODEL_SHA256: &str =
    "c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63";

/// Extra categories tuned for this wallpaper library.
/// These are blended from base CLIP categories to avoid regenerating embeddings.
#[cfg(feature = "clip")]
const LIBRARY_CATEGORY_MIXES: &[(&str, &[(&str, f32)])] = &[
    // ── Composite categories built from base embeddings ──
    (
        "pixel_art",
        &[
            ("retro", 0.35),
            ("vibrant", 0.25),
            ("minimal", 0.20),
            ("geometric", 0.20),
        ],
    ),
    (
        "anime_character",
        &[
            ("anime", 0.50),
            ("portrait", 0.25),
            ("illustration", 0.15),
            ("vibrant", 0.10),
        ],
    ),
    (
        "fantasy_landscape",
        &[
            ("fantasy", 0.40),
            ("nature", 0.25),
            ("mountain", 0.15),
            ("dramatic", 0.10),
            ("landscape_orientation", 0.10),
        ],
    ),
    (
        "epic_battle",
        &[
            ("fantasy", 0.30),
            ("dramatic", 0.30),
            ("dark", 0.15),
            ("samurai", 0.15),
            ("vibrant", 0.10),
        ],
    ),
    (
        "sakura",
        &[
            ("flowers", 0.30),
            ("anime", 0.25),
            ("pastel", 0.25),
            ("serene", 0.20),
        ],
    ),
    (
        "nightscape",
        &[
            ("dark", 0.35),
            ("space", 0.25),
            ("city", 0.20),
            ("neon", 0.20),
        ],
    ),
    (
        "painterly",
        &[
            ("oil_painting", 0.35),
            ("watercolor", 0.25),
            ("fantasy", 0.20),
            ("nature", 0.10),
            ("vintage", 0.10),
        ],
    ),
    (
        "concept_art",
        &[
            ("digital_art", 0.35),
            ("fantasy", 0.25),
            ("dramatic", 0.20),
            ("illustration", 0.20),
        ],
    ),
    (
        "ethereal",
        &[
            ("pastel", 0.30),
            ("serene", 0.25),
            ("fantasy", 0.25),
            ("bright", 0.20),
        ],
    ),
    (
        "moody_fantasy",
        &[
            ("dark", 0.30),
            ("fantasy", 0.30),
            ("gothic", 0.20),
            ("forest", 0.10),
            ("mountain", 0.10),
        ],
    ),
];

/// Model cache directory manager
#[cfg(feature = "clip")]
pub struct ModelManager {
    cache_dir: PathBuf,
    visual_model_url: String,
    visual_model_sha256: Option<String>,
}

#[cfg(feature = "clip")]
impl ModelManager {
    pub fn new(config: &Config) -> Self {
        let cache_dir = project_cache_dir(PathBuf::from("/tmp/frostwall")).join("models");

        let configured_url = config
            .clip
            .visual_model_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
            .map(str::to_string);
        let visual_model_url = configured_url
            .clone()
            .unwrap_or_else(|| DEFAULT_VISUAL_MODEL_URL.to_string());

        let visual_model_sha256 = config
            .clip
            .visual_model_sha256
            .as_deref()
            .map(str::trim)
            .filter(|hash| !hash.is_empty())
            .map(str::to_string)
            .or_else(|| {
                if configured_url.is_none() || visual_model_url == DEFAULT_VISUAL_MODEL_URL {
                    Some(DEFAULT_VISUAL_MODEL_SHA256.to_string())
                } else {
                    None
                }
            });

        Self {
            cache_dir,
            visual_model_url,
            visual_model_sha256,
        }
    }

    fn visual_model_path(&self) -> PathBuf {
        self.cache_dir.join("clip_visual.onnx")
    }

    pub async fn ensure_models(&self) -> Result<PathBuf> {
        std::fs::create_dir_all(&self.cache_dir)?;

        let visual_path = self.visual_model_path();
        let expected_sha256 = self.visual_model_sha256.as_deref();
        let checksum_enabled = expected_sha256.is_some();
        if !checksum_enabled {
            eprintln!(
                "WARNING: CLIP model checksum verification is disabled (custom URL without SHA256)"
            );
        }

        if visual_path.exists() {
            if let Some(expected) = expected_sha256 {
                if !Self::verify_checksum(&visual_path, expected)? {
                    eprintln!("WARNING: Model checksum mismatch — re-downloading...");
                    std::fs::remove_file(&visual_path)?;
                    self.download_model(&self.visual_model_url, &visual_path, "visual encoder")
                        .await?;
                    if !Self::verify_checksum(&visual_path, expected)? {
                        anyhow::bail!("Downloaded model failed checksum verification");
                    }
                }
            }
        } else {
            self.download_model(&self.visual_model_url, &visual_path, "visual encoder")
                .await?;
            if let Some(expected) = expected_sha256 {
                if !Self::verify_checksum(&visual_path, expected)? {
                    std::fs::remove_file(&visual_path)?;
                    anyhow::bail!("Downloaded model failed checksum verification");
                }
            }
        }

        Ok(visual_path)
    }

    fn verify_checksum(path: &Path, expected_hex: &str) -> Result<bool> {
        let mut file = std::fs::File::open(path)?;
        let mut hasher = Sha256::new();
        std::io::copy(&mut file, &mut hasher)?;
        let hash = format!("{:x}", hasher.finalize());
        Ok(hash == expected_hex)
    }

    async fn download_model(&self, url: &str, dest: &Path, name: &str) -> Result<()> {
        eprintln!("Downloading CLIP {} model...", name);

        let client = reqwest::Client::new();
        let response = client
            .get(url)
            .send()
            .await
            .context("Failed to start download")?;

        let total_size = response.content_length().unwrap_or(0);

        let pb = ProgressBar::new(total_size);
        if let Ok(style) = ProgressStyle::default_bar()
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .map(|style| style.progress_chars("#>-"))
        {
            pb.set_style(style);
        }

        let mut file = std::fs::File::create(dest)?;
        let mut stream = response.bytes_stream();
        let mut downloaded: u64 = 0;

        while let Some(chunk) = stream.next().await {
            let chunk = chunk.context("Error downloading chunk")?;
            file.write_all(&chunk)?;
            downloaded += chunk.len() as u64;
            pb.set_position(downloaded);
        }

        pb.finish_with_message("Download complete");
        eprintln!("Saved to {}", dest.display());

        Ok(())
    }
}

/// CLIP inference engine for tagging images
#[cfg(feature = "clip")]
pub struct ClipTagger {
    visual_session: Session,
    category_embeddings: Vec<(String, Vec<f32>)>,
    thumbnail_lookup: ThumbnailLookup,
    batch_size: usize,
    gpu_accelerated: bool,
}

#[cfg(feature = "clip")]
pub struct ClipAnalysis {
    pub tags: Vec<AutoTag>,
    pub embedding: Vec<f32>,
}

#[cfg(feature = "clip")]
impl ClipTagger {
    /// Create a new tagger by loading ONNX models
    pub async fn new(config: &Config) -> Result<Self> {
        let model_manager = ModelManager::new(config);
        let visual_path = model_manager.ensure_models().await?;

        eprintln!("Loading CLIP visual model...");

        // Try CUDA first, fall back to CPU
        #[cfg(feature = "clip-cuda")]
        let (visual_session, gpu_accelerated) = {
            use ort::execution_providers::{CUDAExecutionProvider, ExecutionProvider};

            let cuda_available = CUDAExecutionProvider::default().is_available()?;

            if cuda_available {
                eprintln!("Using CUDA GPU acceleration");
                (
                    Session::builder()?
                        .with_execution_providers([CUDAExecutionProvider::default().build()])?
                        .commit_from_file(&visual_path)
                        .context("Failed to load visual model with CUDA")?,
                    true,
                )
            } else {
                eprintln!("CUDA not available, using CPU");
                (
                    Session::builder()?
                        .with_intra_threads(4)?
                        .commit_from_file(&visual_path)
                        .context("Failed to load visual model")?,
                    false,
                )
            }
        };

        #[cfg(not(feature = "clip-cuda"))]
        let (visual_session, gpu_accelerated) = (
            Session::builder()?
                .with_intra_threads(4)?
                .commit_from_file(&visual_path)
                .context("Failed to load visual model")?,
            false,
        );

        eprintln!("CLIP model loaded successfully");

        Ok(Self {
            visual_session,
            category_embeddings: build_category_embeddings(),
            thumbnail_lookup: ThumbnailLookup::new(
                config.thumbnails.width,
                config.thumbnails.height,
                config.thumbnails.quality,
            ),
            batch_size: config.clip.batch_size.max(1),
            gpu_accelerated,
        })
    }

    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    pub fn is_gpu_accelerated(&self) -> bool {
        self.gpu_accelerated
    }

    pub fn analyze_images_batch_verbose(
        &mut self,
        image_paths: &[PathBuf],
        threshold: f32,
        verbose_first: bool,
    ) -> Vec<Result<ClipAnalysis>> {
        if image_paths.is_empty() {
            return Vec::new();
        }

        let lookup = &self.thumbnail_lookup;
        let mut results: Vec<Option<Result<ClipAnalysis>>> = std::iter::repeat_with(|| None)
            .take(image_paths.len())
            .collect();
        let preprocessed: Vec<_> = image_paths
            .par_iter()
            .map(|path| preprocess_image_with_lookup(lookup, path))
            .collect();
        let mut successful = Vec::with_capacity(image_paths.len());

        for (index, prepared) in preprocessed.into_iter().enumerate() {
            match prepared {
                Ok(input) => successful.push((index, input)),
                Err(err) => results[index] = Some(Err(err)),
            }
        }

        if !successful.is_empty() {
            let batch_result = self.run_batch_inference(&successful, threshold, verbose_first);
            match batch_result {
                Ok(analyses) => {
                    for ((index, _), analysis) in successful.into_iter().zip(analyses) {
                        results[index] = Some(Ok(analysis));
                    }
                }
                Err(err) => {
                    let message = format!("{err:#}");
                    for (index, _) in successful {
                        results[index] = Some(Err(anyhow::anyhow!(message.clone())));
                    }
                }
            }
        }

        results
            .into_iter()
            .map(|result| result.expect("batch analysis should populate every result"))
            .collect()
    }

    fn run_batch_inference(
        &mut self,
        prepared: &[(usize, Vec<f32>)],
        threshold: f32,
        verbose_first: bool,
    ) -> Result<Vec<ClipAnalysis>> {
        let batch_len = prepared.len();
        let mut input_data = Vec::with_capacity(batch_len * CLIP_SAMPLE_LEN);
        for (_, input) in prepared {
            input_data.extend_from_slice(input);
        }

        let input_tensor = ort::value::Tensor::<f32>::from_array((
            [
                batch_len,
                CLIP_CHANNELS,
                CLIP_IMAGE_SIZE as usize,
                CLIP_IMAGE_SIZE as usize,
            ],
            input_data,
        ))?;
        let embeddings = {
            let outputs = self.visual_session.run(ort::inputs![input_tensor])?;
            let (_, output_value) = outputs.iter().next().context("No output tensor found")?;
            let tensor_ref = output_value
                .try_extract_tensor::<f32>()
                .context("Failed to extract embedding tensor")?;

            let shape: Vec<usize> = tensor_ref.0.iter().map(|&x| x as usize).collect();
            let embedding_data: &[f32] = tensor_ref.1;

            if verbose_first {
                eprintln!("  Output shape: {:?}", shape);
                eprintln!("  Output data length: {}", embedding_data.len());
            }

            extract_batch_embeddings(&shape, embedding_data, batch_len, verbose_first)?
        };
        let analyses = embeddings
            .into_iter()
            .enumerate()
            .map(|(index, embedding)| {
                self.build_analysis(embedding, threshold, verbose_first && index == 0)
            })
            .collect();
        Ok(analyses)
    }

    fn build_analysis(&self, embedding: Vec<f32>, threshold: f32, verbose: bool) -> ClipAnalysis {
        if verbose {
            eprintln!("  Embedding dimension: {}", embedding.len());
            eprintln!("  Expected dimension: {}", EMBEDDING_DIM);
            eprintln!(
                "  First 5 values: {:?}",
                &embedding[..5.min(embedding.len())]
            );
        }

        let projected = if embedding.len() != EMBEDDING_DIM {
            eprintln!(
                "WARNING: embedding dim {} != expected {}! Model may be incompatible.",
                embedding.len(),
                EMBEDDING_DIM
            );
            embedding
        } else {
            embedding
        };

        let normalized = normalize_embedding(projected);
        let mut tags = Vec::new();
        let mut all_scores: Vec<(&str, f32, f32)> = Vec::new();

        for (name, cat_embedding) in &self.category_embeddings {
            let similarity: f32 = if normalized.len() == cat_embedding.len() {
                normalized
                    .iter()
                    .zip(cat_embedding.iter())
                    .map(|(a, b)| a * b)
                    .sum()
            } else {
                0.0
            };

            let confidence = (similarity + 1.0) / 2.0;
            all_scores.push((name, similarity, confidence));

            if confidence >= threshold {
                tags.push(AutoTag {
                    name: name.clone(),
                    confidence,
                });
            }
        }

        if verbose {
            eprintln!("  Raw similarities (top 5):");
            let mut sorted_scores = all_scores;
            sorted_scores
                .sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            for (name, sim, conf) in sorted_scores.iter().take(5) {
                eprintln!("    {}: raw={:.4}, conf={:.4}", name, sim, conf);
            }
            eprintln!("  Tags above threshold {}: {}", threshold, tags.len());
        }

        tags.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        ClipAnalysis {
            tags,
            embedding: normalized,
        }
    }

    /// Get list of available tag categories
    pub fn available_tags() -> Vec<String> {
        let mut tags: Vec<String> = category_embeddings()
            .iter()
            .map(|(name, _)| name.clone())
            .collect();
        tags.extend(
            LIBRARY_CATEGORY_MIXES
                .iter()
                .map(|(name, _)| name.to_string()),
        );
        tags.sort_unstable();
        tags.dedup();
        tags
    }
}

#[cfg(feature = "clip")]
fn find_base_embedding(name: &str) -> Option<&'static [f32; EMBEDDING_DIM]> {
    category_embeddings()
        .iter()
        .find(|(base_name, _)| base_name == name)
        .map(|(_, embedding)| embedding)
}

#[cfg(feature = "clip")]
fn normalize_embedding(mut embedding: Vec<f32>) -> Vec<f32> {
    let norm = embedding.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for value in &mut embedding {
            *value /= norm;
        }
    }
    embedding
}

#[cfg(feature = "clip")]
fn build_mixed_embedding(parts: &[(&str, f32)]) -> Option<Vec<f32>> {
    let mut mixed = vec![0.0f32; EMBEDDING_DIM];
    let mut total_weight = 0.0f32;

    for (base_name, weight) in parts {
        let Some(base) = find_base_embedding(base_name) else {
            continue;
        };
        for (idx, value) in base.iter().enumerate() {
            mixed[idx] += *value * *weight;
        }
        total_weight += *weight;
    }

    if total_weight <= 0.0 {
        return None;
    }

    for value in &mut mixed {
        *value /= total_weight;
    }
    Some(normalize_embedding(mixed))
}

#[cfg(feature = "clip")]
fn build_category_embeddings() -> Vec<(String, Vec<f32>)> {
    let mut categories: Vec<(String, Vec<f32>)> = category_embeddings()
        .iter()
        .map(|(name, embedding)| (name.clone(), embedding.to_vec()))
        .collect();

    for (name, parts) in LIBRARY_CATEGORY_MIXES {
        if let Some(embedding) = build_mixed_embedding(parts) {
            categories.push((name.to_string(), embedding));
        }
    }

    categories
}

#[cfg(feature = "clip")]
fn preprocess_image_with_lookup(
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

    let mean = [0.481_454_66, 0.457_827_5, 0.408_210_73];
    let std = [0.268_629_54, 0.261_302_6, 0.275_777_1];
    let mut data = Vec::with_capacity(CLIP_SAMPLE_LEN);

    for channel in 0..CLIP_CHANNELS {
        for y in 0..CLIP_IMAGE_SIZE {
            for x in 0..CLIP_IMAGE_SIZE {
                let pixel = rgb.get_pixel(x, y);
                let value = (pixel[channel] as f32 / 255.0 - mean[channel]) / std[channel];
                data.push(value);
            }
        }
    }

    Ok(data)
}

#[cfg(feature = "clip")]
fn extract_batch_embeddings(
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

#[cfg(all(test, feature = "clip"))]
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
