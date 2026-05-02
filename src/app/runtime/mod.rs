mod event_loop;
mod input;
mod workers;

use self::event_loop::run_app;
use self::workers::{analysis_worker, input_worker, thumbnail_worker};
use super::{AnalysisRequest, App, AppEvent, ThumbnailRequest};
use crate::thumbnail::{effective_thumbnail_bounds, ThumbnailCache};
use anyhow::Result;
use crossterm::{
    event, execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

const THUMBNAIL_REQUEST_QUEUE_CAPACITY: usize = 512;
const ANALYSIS_REQUEST_QUEUE_CAPACITY: usize = 4096;
const APP_EVENT_QUEUE_CAPACITY: usize = 1024;
pub(super) const THUMBNAIL_REDRAW_INTERVAL: Duration = Duration::from_millis(16);

pub async fn run_tui(wallpaper_dir: PathBuf) -> Result<()> {
    // Print loading indicator before blocking on cache load.
    // This is printed to the normal terminal (before EnterAlternateScreen)
    // and disappears automatically when TUI takes over.
    eprintln!("Loading wallpaper library…");
    let mut app = App::new(wallpaper_dir)?;

    // Show terminal optimization hint if first run in Kitty.
    if let Some(hint) = app.config.check_terminal_hint() {
        println!("\n{}\n", hint);
        // Wait for keypress.
        enable_raw_mode()?;
        let _ = event::read();
        disable_raw_mode()?;
    }

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(
        stdout,
        EnterAlternateScreen,
        crossterm::event::EnableMouseCapture
    )?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    if let Err(err) = app.init_screens().await {
        let _ = restore_terminal(&mut terminal);
        return Err(err);
    }

    // Set up channels for background thumbnail loading.
    // Bounded queue prevents unlimited backlog during rapid scrolling.
    let (thumb_tx, thumb_rx) =
        mpsc::sync_channel::<ThumbnailRequest>(THUMBNAIL_REQUEST_QUEUE_CAPACITY);
    let (analysis_tx, analysis_rx) =
        mpsc::sync_channel::<AnalysisRequest>(ANALYSIS_REQUEST_QUEUE_CAPACITY);
    // Bounded event queue avoids unbounded memory growth during heavy thumbnail churn.
    let (event_tx, event_rx) = mpsc::sync_channel::<AppEvent>(APP_EVENT_QUEUE_CAPACITY);

    app.set_thumb_channel(thumb_tx);
    app.set_analysis_channel(analysis_tx);

    // Spawn thumbnail worker thread.
    let event_tx_thumb = event_tx.clone();
    let thumb_cfg = &app.config.thumbnails;
    let (thumb_w, thumb_h) = effective_thumbnail_bounds(thumb_cfg.width, thumb_cfg.height);
    let disk_cache = ThumbnailCache::new_with_settings(thumb_w, thumb_h, thumb_cfg.quality);
    thread::spawn(move || {
        thumbnail_worker(thumb_rx, event_tx_thumb, disk_cache);
    });

    // Spawn background color-analysis worker so cold starts can render
    // immediately and fill in palette/similarity data progressively.
    let event_tx_analysis = event_tx.clone();
    thread::spawn(move || {
        analysis_worker(analysis_rx, event_tx_analysis);
    });

    // Spawn event polling thread.
    let event_tx_input = event_tx.clone();
    thread::spawn(move || {
        input_worker(event_tx_input);
    });

    app.queue_initial_thumbnail_warmup();
    let res = run_app(&mut terminal, &mut app, event_rx);

    restore_terminal(&mut terminal)?;

    app.persist_last_selection();
    app.cache.save()?;
    app.config.save()?;

    res
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        crossterm::event::DisableMouseCapture
    )?;
    terminal.show_cursor()?;
    Ok(())
}
