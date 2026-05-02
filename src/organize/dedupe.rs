use super::display_name;
use super::plan::WallpaperEntry;
use anyhow::{Context, Result};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

pub(super) fn detect_duplicate_numbered_stems(entries: &[WallpaperEntry]) -> Vec<String> {
    let mut seen: HashMap<(super::bucket::RenameBucket, u32), PathBuf> = HashMap::new();
    let mut warnings = Vec::new();

    for entry in entries {
        let Some((bucket, number)) = entry.current_name else {
            continue;
        };
        let key = (bucket, number);
        if let Some(first) = seen.get(&key) {
            warnings.push(format!(
                "{}-wallpaper{}: {} <-> {}",
                bucket.prefix(),
                number,
                display_name(first),
                display_name(&entry.path)
            ));
        } else {
            seen.insert(key, entry.path.clone());
        }
    }

    warnings
}

pub(super) fn detect_content_duplicates(entries: &[WallpaperEntry]) -> Vec<String> {
    let hashes: Vec<_> = entries
        .par_iter()
        .filter_map(|entry| {
            file_sha256(&entry.path)
                .ok()
                .map(|hash| (hash, entry.path.clone()))
        })
        .collect();

    let mut first_by_hash: HashMap<String, PathBuf> = HashMap::new();
    let mut warnings = Vec::new();

    for (hash, path) in hashes {
        if let Some(first) = first_by_hash.get(&hash) {
            warnings.push(format!(
                "{} == {}",
                display_name(first),
                display_name(&path)
            ));
        } else {
            first_by_hash.insert(hash, path);
        }
    }

    warnings
}

fn file_sha256(path: &Path) -> Result<String> {
    let mut file =
        fs::File::open(path).with_context(|| format!("Failed to open {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 8192];

    loop {
        let read = file
            .read(&mut buffer)
            .with_context(|| format!("Failed to read {}", path.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(format!("{:x}", hasher.finalize()))
}
