use crate::app::Config;
use crate::screen;
use crate::swww;
use crate::timeprofile::TimePeriod;
use crate::wallpaper::WallpaperCache;
use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

const FS_RESCAN_DEBOUNCE: Duration = Duration::from_millis(1200);

/// Watch daemon configuration
pub struct WatchConfig {
    pub interval: Duration,
    pub shuffle: bool,
    pub watch_dir: bool,
}

impl Default for WatchConfig {
    fn default() -> Self {
        Self {
            interval: Duration::from_secs(30 * 60), // 30 minutes
            shuffle: true,
            watch_dir: true,
        }
    }
}

/// Parse interval string like "30m", "1h", "90s"
pub fn parse_interval(s: &str) -> Option<Duration> {
    let s = s.trim().to_lowercase();

    if let Some(mins) = s.strip_suffix('m') {
        return mins
            .parse::<u64>()
            .ok()
            .map(|m| Duration::from_secs(m * 60));
    }
    if let Some(hours) = s.strip_suffix('h') {
        return hours
            .parse::<u64>()
            .ok()
            .map(|h| Duration::from_secs(h * 3600));
    }
    if let Some(secs) = s.strip_suffix('s') {
        return secs.parse::<u64>().ok().map(Duration::from_secs);
    }

    // Plain number = minutes
    s.parse::<u64>().ok().map(|m| Duration::from_secs(m * 60))
}

/// Run the watch daemon
pub async fn run_watch(watch_config: WatchConfig) -> Result<()> {
    let config = Config::load()?;
    let recursive = config.wallpaper.recursive;
    let wallpaper_dir = config.wallpaper_dir();

    println!("❄️  FrostWall Watch Daemon");
    println!("   Directory: {}", wallpaper_dir.display());
    println!("   Interval:  {} seconds", watch_config.interval.as_secs());
    println!("   Shuffle:   {}", watch_config.shuffle);
    println!("   Watching:  {}", watch_config.watch_dir);
    println!();

    // Set up graceful shutdown
    let running = Arc::new(AtomicBool::new(true));
    let running_clone = running.clone();

    // Handle Ctrl+C
    ctrlc_handler(running_clone);

    // Initial scan
    let mut cache = WallpaperCache::load_or_scan(
        &wallpaper_dir,
        recursive,
        crate::wallpaper::CacheLoadMode::Full,
    )?;
    println!("✓ Loaded {} wallpapers", cache.wallpapers.len());

    // Set up file system watcher
    let (fs_tx, fs_rx) = mpsc::channel();
    let mut _watcher: Option<RecommendedWatcher> = None;

    if watch_config.watch_dir {
        match RecommendedWatcher::new(
            move |res| {
                if let Ok(event) = res {
                    let _ = fs_tx.send(event);
                }
            },
            NotifyConfig::default(),
        ) {
            Ok(mut w) => {
                let mode = if recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };
                if let Err(e) = w.watch(&wallpaper_dir, mode) {
                    eprintln!("⚠ Could not watch directory: {}", e);
                } else {
                    println!("✓ Watching for file changes");
                    _watcher = Some(w);
                }
            }
            Err(e) => {
                eprintln!("⚠ Could not create watcher: {}", e);
            }
        }
    }

    // Detect screens
    let screens = screen::detect_screens().await?;
    if screens.is_empty() {
        anyhow::bail!("No screens detected");
    }
    println!("✓ Found {} screen(s)", screens.len());

    // Set initial wallpaper
    set_wallpapers(&mut cache, &screens, &config, watch_config.shuffle)?;

    let mut last_change = Instant::now();
    let mut cache_dirty = false;
    let mut last_fs_change_at: Option<Instant> = None;

    println!("\n🔄 Running... (Ctrl+C to stop)\n");

    while running.load(Ordering::SeqCst) {
        // Check for file system events
        while let Ok(event) = fs_rx.try_recv() {
            use notify::EventKind;

            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) => {
                    // Check if it's an image file
                    for path in &event.paths {
                        if is_image_file(path) {
                            println!("📁 File change detected: {}", path.display());
                            cache_dirty = true;
                            last_fs_change_at = Some(Instant::now());
                        }
                    }
                }
                _ => {}
            }
        }

        // Run a debounced incremental refresh to avoid expensive full rescans
        // while batch copies/moves are still in progress.
        if cache_dirty
            && last_fs_change_at
                .map(|t| t.elapsed() >= FS_RESCAN_DEBOUNCE)
                .unwrap_or(false)
        {
            println!("🔄 Refreshing cache (incremental)...");
            match cache.incremental_rescan(recursive) {
                Ok((added, removed)) => {
                    let total = cache.wallpapers.len();
                    if added > 0 || removed > 0 {
                        println!("✓ +{} / -{} wallpapers (total: {})", added, removed, total);
                    } else {
                        println!("✓ Cache verified ({} wallpapers)", total);
                    }
                }
                Err(e) => {
                    eprintln!("⚠ Failed to refresh cache: {}", e);
                }
            }
            cache_dirty = false;
            last_fs_change_at = None;
        }

        // Check if it's time to change wallpaper
        if last_change.elapsed() >= watch_config.interval {
            println!("⏰ Interval elapsed, changing wallpaper...");
            set_wallpapers(&mut cache, &screens, &config, watch_config.shuffle)?;
            last_change = Instant::now();
        }

        // Sleep a bit before next check without blocking the async runtime
        tokio::time::sleep(Duration::from_millis(500)).await;
    }

    // Graceful shutdown
    println!("\n❄️  Shutting down gracefully...");
    drop(_watcher);
    cache.save()?;
    println!("✓ Cache saved. Goodbye!");

    Ok(())
}

/// Set up Ctrl+C handler
fn ctrlc_handler(running: Arc<AtomicBool>) {
    // Use tokio's signal handling
    tokio::spawn(async move {
        if let Ok(()) = tokio::signal::ctrl_c().await {
            running.store(false, Ordering::SeqCst);
        }
    });
}

fn set_wallpapers(
    cache: &mut WallpaperCache,
    screens: &[screen::Screen],
    config: &Config,
    shuffle: bool,
) -> Result<()> {
    if !shuffle {
        for screen in screens {
            if let Some(wp) = cache.next_for_screen(screen) {
                swww::set_wallpaper_with_resize(
                    &screen.name,
                    &wp.path,
                    &config.transition(),
                    config.display.resize_mode,
                    &config.display.fill_color,
                )
                .with_context(|| format!("Failed to set wallpaper on {}", screen.name))?;

                println!(
                    "  {} → {}",
                    screen.name,
                    wp.path.file_name().unwrap_or_default().to_string_lossy()
                );
            }
        }
        return Ok(());
    }

    // Check if time profiles are enabled
    let use_time_profiles = config.time_profiles.enabled;
    let period = TimePeriod::current();

    if use_time_profiles {
        println!("  {} Time period: {}", period.emoji(), period.name());
    }

    for screen in screens {
        let wp_path = if use_time_profiles {
            // Get wallpapers sorted by time profile score
            let suitable: Vec<(usize, f32)> = cache
                .wallpapers
                .iter()
                .enumerate()
                .filter(|(_, wp)| !wp.colors.is_empty())
                .filter(|(_, wp)| wp.matches_screen(screen))
                .map(|(idx, wp)| {
                    let score = config.time_profiles.score_wallpaper(&wp.colors, &wp.tags);
                    (idx, score)
                })
                .filter(|(_, score)| *score >= 0.4) // Minimum threshold
                .collect();

            if suitable.is_empty() {
                // Fallback to random if no suitable wallpapers
                cache.random_for_screen(screen).map(|wp| wp.path.clone())
            } else {
                // Pick randomly from top 20% of scored wallpapers
                let top_count = (suitable.len() / 5).max(3).min(suitable.len());
                let mut sorted = suitable;
                sorted.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

                use rand::seq::SliceRandom;
                sorted[..top_count]
                    .choose(&mut rand::thread_rng())
                    .and_then(|(idx, _)| cache.wallpapers.get(*idx))
                    .map(|wp| wp.path.clone())
            }
        } else {
            cache.random_for_screen(screen).map(|wp| wp.path.clone())
        };

        if let Some(wp_path) = wp_path {
            swww::set_wallpaper_with_resize(
                &screen.name,
                &wp_path,
                &config.transition(),
                config.display.resize_mode,
                &config.display.fill_color,
            )
            .with_context(|| format!("Failed to set wallpaper on {}", screen.name))?;

            println!(
                "  {} → {}",
                screen.name,
                wp_path.file_name().unwrap_or_default().to_string_lossy()
            );
        }
    }
    Ok(())
}

fn is_image_file(path: &Path) -> bool {
    crate::utils::is_image_file(path)
}
