//! CLIP visual-encoder model lifecycle: cache directory layout, downloading
//! from a configured URL, and SHA256 verification.

use anyhow::{Context, Result};
use futures_util::StreamExt;
use indicatif::{ProgressBar, ProgressStyle};
use sha2::{Digest, Sha256};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::app::Config;
use crate::utils::project_cache_dir;

/// Default model URL: Qdrant's CLIP ViT-B/32 vision encoder, which exposes
/// the projected 512-dim embedding head.
const DEFAULT_VISUAL_MODEL_URL: &str =
    "https://huggingface.co/Qdrant/clip-ViT-B-32-vision/resolve/main/model.onnx";

const DEFAULT_VISUAL_MODEL_SHA256: &str =
    "c68d3d9a200ddd2a8c8a5510b576d4c94d1ae383bf8b36dd8c084f94e1fb4d63";

pub struct ModelManager {
    cache_dir: PathBuf,
    visual_model_url: String,
    visual_model_sha256: Option<String>,
}

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
