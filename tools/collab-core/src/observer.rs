use crate::model::{ObserverFrameInput, now_unix_ms};
use crate::state::SharedState;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{debug, warn};

#[derive(Debug, Clone)]
pub struct ObserverConfig {
    pub frames_dir: PathBuf,
    pub scan_interval: Duration,
}

impl Default for ObserverConfig {
    fn default() -> Self {
        Self {
            frames_dir: PathBuf::from("/tmp/displayfrost-observer/frames"),
            scan_interval: Duration::from_millis(800),
        }
    }
}

pub fn spawn_watcher(state: SharedState, config: ObserverConfig) {
    tokio::spawn(async move {
        let mut seen = prime_seen(&config.frames_dir);
        loop {
            match scan_new_frames(&config.frames_dir, &mut seen) {
                Ok(frames) => {
                    for frame in frames {
                        let observed = frame.filename.clone();
                        state.record_observer_frame(frame);
                        debug!("observer frame ingested: {observed}");
                    }
                }
                Err(err) => {
                    warn!(
                        "observer frame scan failed for {}: {err}",
                        config.frames_dir.display()
                    );
                }
            }
            tokio::time::sleep(config.scan_interval).await;
        }
    });
}

fn prime_seen(frames_dir: &Path) -> HashSet<String> {
    let mut seen = HashSet::new();
    let Ok(entries) = fs::read_dir(frames_dir) else {
        return seen;
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file()
            && is_supported_image(&path)
            && let Some(key) = path.to_str()
        {
            seen.insert(key.to_string());
        }
    }
    seen
}

fn scan_new_frames(
    frames_dir: &Path,
    seen: &mut HashSet<String>,
) -> std::io::Result<Vec<ObserverFrameInput>> {
    let mut fresh: Vec<(u64, ObserverFrameInput)> = Vec::new();
    if !frames_dir.exists() {
        return Ok(Vec::new());
    }

    for entry_result in fs::read_dir(frames_dir)? {
        let entry = entry_result?;
        let path = entry.path();
        if !path.is_file() || !is_supported_image(&path) {
            continue;
        }

        let Some(path_str) = path.to_str() else {
            continue;
        };
        if seen.contains(path_str) {
            continue;
        }

        let metadata = entry.metadata()?;
        let modified_at_ms = metadata
            .modified()
            .map(system_time_to_unix_ms)
            .unwrap_or_else(|_| now_unix_ms());
        let filename = entry.file_name().to_string_lossy().to_string();

        fresh.push((
            modified_at_ms,
            ObserverFrameInput {
                path: path_str.to_string(),
                filename,
                size_bytes: metadata.len(),
                modified_at_ms,
            },
        ));
        seen.insert(path_str.to_string());
    }

    fresh.sort_by_key(|(modified, _)| *modified);
    Ok(fresh.into_iter().map(|(_, frame)| frame).collect())
}

fn is_supported_image(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|value| value.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "webp"
    )
}

fn system_time_to_unix_ms(time: SystemTime) -> u64 {
    time.duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
