use super::gallery::GalleryImage;
use super::WebImporter;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;

const MAX_DOWNLOAD_BYTES: u64 = 100 * 1024 * 1024;

async fn validate_download_response(response: &reqwest::Response) -> Result<()> {
    if !response.status().is_success() {
        anyhow::bail!("Download failed with status: {}", response.status());
    }

    if let Some(content_type) = response.headers().get(reqwest::header::CONTENT_TYPE) {
        let is_image = content_type
            .to_str()
            .map(|value| value.starts_with("image/"))
            .unwrap_or(false);
        if !is_image {
            let content_type_str = String::from_utf8_lossy(content_type.as_bytes());
            anyhow::bail!("Unexpected content type: {}", content_type_str);
        }
    }

    if let Some(content_len) = response.content_length() {
        if content_len > MAX_DOWNLOAD_BYTES {
            anyhow::bail!(
                "Image too large: {} bytes (max {})",
                content_len,
                MAX_DOWNLOAD_BYTES
            );
        }
    }

    Ok(())
}

async fn stream_download_to_file(mut response: reqwest::Response, tmp_path: &Path) -> Result<()> {
    let mut file = fs::File::create(tmp_path)
        .await
        .with_context(|| format!("Failed to create temp file: {}", tmp_path.display()))?;

    let mut written: u64 = 0;
    while let Some(chunk) = response
        .chunk()
        .await
        .context("Failed to read image data")?
    {
        written += chunk.len() as u64;
        if written > MAX_DOWNLOAD_BYTES {
            let _ = fs::remove_file(tmp_path).await;
            anyhow::bail!(
                "Image exceeded max download size ({} bytes)",
                MAX_DOWNLOAD_BYTES
            );
        }

        file.write_all(&chunk)
            .await
            .with_context(|| format!("Failed to write {}", tmp_path.display()))?;
    }

    file.flush()
        .await
        .with_context(|| format!("Failed to flush {}", tmp_path.display()))?;

    Ok(())
}

fn temp_download_path(dest_path: &Path, extension: &str) -> PathBuf {
    dest_path.with_extension(format!("{extension}.part"))
}

impl WebImporter {
    /// Download an image to the specified directory.
    pub async fn download(&self, image: &GalleryImage, dest_dir: &Path) -> Result<PathBuf> {
        let extension = image.download_extension();
        let dest_path = dest_dir.join(image.download_filename());

        if dest_path.exists() {
            return Ok(dest_path);
        }

        let response = self
            .client
            .get(&image.url)
            .send()
            .await
            .context("Failed to download image")?;
        validate_download_response(&response).await?;

        fs::create_dir_all(dest_dir).await?;

        let tmp_path = temp_download_path(&dest_path, extension);
        stream_download_to_file(response, &tmp_path).await?;

        image::image_dimensions(&tmp_path).with_context(|| {
            format!(
                "Downloaded file is not a valid image: {}",
                tmp_path.display()
            )
        })?;

        fs::rename(&tmp_path, &dest_path)
            .await
            .with_context(|| format!("Failed to finalize image file: {}", dest_path.display()))?;

        Ok(dest_path)
    }
}
