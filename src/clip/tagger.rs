//! CLIP inference engine: loads the visual encoder, batches preprocessed
//! images through ONNX Runtime, then scores each embedding against the
//! category catalog to produce confidence-ranked tags.

use anyhow::{Context, Result};
use ort::session::Session;
use rayon::prelude::*;
use std::path::PathBuf;

use super::categories::{
    build_category_embeddings, normalize_embedding, LIBRARY_CATEGORY_MIXES,
};
use super::model::ModelManager;
use super::preprocess::{
    extract_batch_embeddings, preprocess_image_with_lookup, CLIP_CHANNELS, CLIP_IMAGE_SIZE,
    CLIP_SAMPLE_LEN,
};
use super::AutoTag;
use crate::app::Config;
use crate::clip_embeddings_bin::{category_embeddings, EMBEDDING_DIM};
use crate::thumbnail::ThumbnailLookup;

pub struct ClipTagger {
    visual_session: Session,
    category_embeddings: Vec<(String, Vec<f32>)>,
    thumbnail_lookup: ThumbnailLookup,
    batch_size: usize,
    gpu_accelerated: bool,
}

pub struct ClipAnalysis {
    pub tags: Vec<AutoTag>,
    pub embedding: Vec<f32>,
}

impl ClipTagger {
    /// Create a new tagger by loading the ONNX visual model.
    pub async fn new(config: &Config) -> Result<Self> {
        let model_manager = ModelManager::new(config);
        let visual_path = model_manager.ensure_models().await?;

        eprintln!("Loading CLIP visual model...");

        // Try CUDA first, fall back to CPU.
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

        if embedding.len() != EMBEDDING_DIM {
            eprintln!(
                "WARNING: embedding dim {} != expected {}! Model may be incompatible.",
                embedding.len(),
                EMBEDDING_DIM
            );
        }

        let normalized = normalize_embedding(embedding);
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

    /// Get the list of available tag categories (base + library mixes).
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
