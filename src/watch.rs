use crate::app::Config;
use crate::screen;
use crate::swww;
use crate::wallpaper::WallpaperCache;
use anyhow::{Context, Result};
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
use std::time::{Duration, Instant};

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
        return mins.parse::<u64>().ok().map(|m| Duration::from_secs(m * 60));
    }
    if let Some(hours) = s.strip_suffix('h') {
        return hours.parse::<u64>().ok().map(|h| Duration::from_secs(h * 3600));
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
    let mut cache = WallpaperCache::load_or_scan(&wallpaper_dir)?;
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
                if let Err(e) = w.watch(&wallpaper_dir, RecursiveMode::NonRecursive) {
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
    set_random_wallpapers(&cache, &screens, &config)?;

    let mut last_change = Instant::now();
    let mut cache_dirty = false;

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
                        }
                    }
                }
                _ => {}
            }
        }

        // Reload cache if dirty
        if cache_dirty {
            println!("🔄 Rescanning wallpaper directory...");
            match WallpaperCache::scan(&wallpaper_dir) {
                Ok(new_cache) => {
                    let old_count = cache.wallpapers.len();
                    let new_count = new_cache.wallpapers.len();
                    cache = new_cache;
                    cache.save()?;

                    if new_count > old_count {
                        println!("✓ Added {} new wallpaper(s) (total: {})", new_count - old_count, new_count);
                    } else if new_count < old_count {
                        println!("✓ Removed {} wallpaper(s) (total: {})", old_count - new_count, new_count);
                    } else {
                        println!("✓ Cache updated ({} wallpapers)", new_count);
                    }
                }
                Err(e) => {
                    eprintln!("⚠ Failed to rescan: {}", e);
                }
            }
            cache_dirty = false;
        }

        // Check if it's time to change wallpaper
        if last_change.elapsed() >= watch_config.interval {
            println!("⏰ Interval elapsed, changing wallpaper...");
            set_random_wallpapers(&cache, &screens, &config)?;
            last_change = Instant::now();
        }

        // Sleep a bit before next check
        std::thread::sleep(Duration::from_millis(500));
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

fn set_random_wallpapers(
    cache: &WallpaperCache,
    screens: &[screen::Screen],
    config: &Config,
) -> Result<()> {
    for screen in screens {
        if let Some(wp) = cache.random_for_screen(screen) {
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
    Ok(())
}

fn is_image_file(path: &Path) -> bool {
    crate::utils::is_image_file(path)
}

/// Run a single wallpaper change (for cron/timer use)
#[allow(dead_code)]
pub async fn run_once(shuffle: bool) -> Result<()> {
    let config = Config::load()?;
    let wallpaper_dir = config.wallpaper_dir();
    let cache = WallpaperCache::load_or_scan(&wallpaper_dir)?;
    let screens = screen::detect_screens().await?;

    if shuffle {
        set_random_wallpapers(&cache, &screens, &config)?;
    } else {
        // Sequential mode - use next_for_screen
        let mut cache = cache;
        for screen in &screens {
            if let Some(wp) = cache.next_for_screen(screen) {
                swww::set_wallpaper_with_resize(
                    &screen.name,
                    &wp.path,
                    &config.transition(),
                    config.display.resize_mode,
                    &config.display.fill_color,
                )?;
                println!("{}: {}", screen.name, wp.path.display());
            }
        }
        cache.save()?;
    }

    Ok(())
}
