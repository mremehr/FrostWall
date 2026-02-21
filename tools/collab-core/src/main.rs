mod api;
mod model;
mod observer;
mod state;

use anyhow::{Context, Result};
use observer::ObserverConfig;
use state::SharedState;
use std::env;
use std::path::PathBuf;
use std::time::Duration;
use tokio::net::TcpListener;
use tracing::info;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    init_tracing();

    let bind = env::var("COLLAB_BIND").unwrap_or_else(|_| "127.0.0.1:7878".to_string());
    let observer_dir = env::var("COLLAB_OBSERVER_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp/frostwall-observer/frames"));
    let observer_scan_ms = env::var("COLLAB_OBSERVER_SCAN_MS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(800);

    let state = SharedState::new();
    let observer_config = ObserverConfig {
        frames_dir: observer_dir.clone(),
        scan_interval: Duration::from_millis(observer_scan_ms),
    };
    observer::spawn_watcher(state.clone(), observer_config);

    let app = api::router(state.clone());
    let listener = TcpListener::bind(&bind)
        .await
        .with_context(|| format!("failed to bind {bind}"))?;

    info!(
        address = %bind,
        observer_dir = %observer_dir.display(),
        observer_scan_ms,
        "collab-core starting"
    );

    state.publish_snapshot();
    axum::serve(listener, app)
        .await
        .context("collab-core server failed")?;

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .compact()
        .init();
}
