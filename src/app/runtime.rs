use super::{App, AppEvent, ThumbnailRequest, ThumbnailResponse};
use crate::thumbnail::{effective_thumbnail_bounds, ThumbnailCache};
use crate::ui;
use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::collections::HashMap;
use std::io;
use std::path::PathBuf;
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::thread;
use std::time::{Duration, Instant};

const THUMBNAIL_REQUEST_QUEUE_CAPACITY: usize = 512;
const APP_EVENT_QUEUE_CAPACITY: usize = 1024;
const PERF_LOG_INTERVAL: Duration = Duration::from_secs(3);
const THUMBNAIL_REDRAW_INTERVAL: Duration = Duration::from_millis(16);

fn perf_enabled() -> bool {
    std::env::var("FROSTWALL_PERF")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

#[derive(Debug)]
struct RuntimePerf {
    enabled: bool,
    window_start: Instant,
    draw_count: u64,
    draw_total: Duration,
    draw_max: Duration,
    event_batches: u64,
    event_total: u64,
    event_process_total: Duration,
    event_process_max: Duration,
    key_events: u64,
    thumbnail_events: u64,
    resize_events: u64,
    tick_events: u64,
}

impl RuntimePerf {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            window_start: Instant::now(),
            draw_count: 0,
            draw_total: Duration::ZERO,
            draw_max: Duration::ZERO,
            event_batches: 0,
            event_total: 0,
            event_process_total: Duration::ZERO,
            event_process_max: Duration::ZERO,
            key_events: 0,
            thumbnail_events: 0,
            resize_events: 0,
            tick_events: 0,
        }
    }

    fn record_draw(&mut self, elapsed: Duration) {
        if !self.enabled {
            return;
        }
        self.draw_count = self.draw_count.saturating_add(1);
        self.draw_total += elapsed;
        self.draw_max = self.draw_max.max(elapsed);
    }

    fn record_event_batch(&mut self, events: usize, process_elapsed: Duration) {
        if !self.enabled {
            return;
        }
        self.event_batches = self.event_batches.saturating_add(1);
        self.event_total = self.event_total.saturating_add(events as u64);
        self.event_process_total += process_elapsed;
        self.event_process_max = self.event_process_max.max(process_elapsed);
    }

    fn record_event(&mut self, event: &AppEvent) {
        if !self.enabled {
            return;
        }
        match event {
            AppEvent::Key(_) => self.key_events = self.key_events.saturating_add(1),
            AppEvent::ThumbnailReady(_) => {
                self.thumbnail_events = self.thumbnail_events.saturating_add(1)
            }
            AppEvent::Resize => self.resize_events = self.resize_events.saturating_add(1),
            AppEvent::Tick => self.tick_events = self.tick_events.saturating_add(1),
        }
    }

    fn maybe_log(&mut self) {
        if !self.enabled || self.window_start.elapsed() < PERF_LOG_INTERVAL {
            return;
        }

        let draw_avg_ms = if self.draw_count > 0 {
            self.draw_total.as_secs_f64() * 1000.0 / self.draw_count as f64
        } else {
            0.0
        };
        let events_per_batch = if self.event_batches > 0 {
            self.event_total as f64 / self.event_batches as f64
        } else {
            0.0
        };
        let process_avg_ms = if self.event_batches > 0 {
            self.event_process_total.as_secs_f64() * 1000.0 / self.event_batches as f64
        } else {
            0.0
        };

        eprintln!(
            "[perf][runtime] draws={} draw_avg={:.2}ms draw_max={:.2}ms batches={} events={} ev/batch={:.2} process_avg={:.2}ms process_max={:.2}ms key={} thumb={} resize={} tick={}",
            self.draw_count,
            draw_avg_ms,
            self.draw_max.as_secs_f64() * 1000.0,
            self.event_batches,
            self.event_total,
            events_per_batch,
            process_avg_ms,
            self.event_process_max.as_secs_f64() * 1000.0,
            self.key_events,
            self.thumbnail_events,
            self.resize_events,
            self.tick_events
        );

        self.window_start = Instant::now();
        self.draw_count = 0;
        self.draw_total = Duration::ZERO;
        self.draw_max = Duration::ZERO;
        self.event_batches = 0;
        self.event_total = 0;
        self.event_process_total = Duration::ZERO;
        self.event_process_max = Duration::ZERO;
        self.key_events = 0;
        self.thumbnail_events = 0;
        self.resize_events = 0;
        self.tick_events = 0;
    }
}

#[derive(Debug)]
struct WorkerPerf {
    enabled: bool,
    window_start: Instant,
    batches: u64,
    requests: u64,
    decode_total: Duration,
    decode_max: Duration,
    failures: u64,
}

impl WorkerPerf {
    fn new(enabled: bool) -> Self {
        Self {
            enabled,
            window_start: Instant::now(),
            batches: 0,
            requests: 0,
            decode_total: Duration::ZERO,
            decode_max: Duration::ZERO,
            failures: 0,
        }
    }

    fn record_batch(&mut self, size: usize) {
        if !self.enabled {
            return;
        }
        self.batches = self.batches.saturating_add(1);
        self.requests = self.requests.saturating_add(size as u64);
    }

    fn record_decode(&mut self, elapsed: Duration, ok: bool) {
        if !self.enabled {
            return;
        }
        self.decode_total += elapsed;
        self.decode_max = self.decode_max.max(elapsed);
        if !ok {
            self.failures = self.failures.saturating_add(1);
        }
    }

    fn maybe_log(&mut self) {
        if !self.enabled || self.window_start.elapsed() < PERF_LOG_INTERVAL {
            return;
        }

        let decode_avg_ms = if self.requests > 0 {
            self.decode_total.as_secs_f64() * 1000.0 / self.requests as f64
        } else {
            0.0
        };

        eprintln!(
            "[perf][thumb-worker] batches={} requests={} decode_avg={:.2}ms decode_max={:.2}ms failures={}",
            self.batches,
            self.requests,
            decode_avg_ms,
            self.decode_max.as_secs_f64() * 1000.0,
            self.failures
        );

        self.window_start = Instant::now();
        self.batches = 0;
        self.requests = 0;
        self.decode_total = Duration::ZERO;
        self.decode_max = Duration::ZERO;
        self.failures = 0;
    }
}

pub async fn run_tui(wallpaper_dir: PathBuf) -> Result<()> {
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
    // Bounded event queue avoids unbounded memory growth during heavy thumbnail churn.
    let (event_tx, event_rx) = mpsc::sync_channel::<AppEvent>(APP_EVENT_QUEUE_CAPACITY);

    app.set_thumb_channel(thumb_tx);

    // Spawn thumbnail worker thread.
    let event_tx_thumb = event_tx.clone();
    let thumb_cfg = &app.config.thumbnails;
    let (thumb_w, thumb_h) = effective_thumbnail_bounds(thumb_cfg.width, thumb_cfg.height);
    let disk_cache = ThumbnailCache::new_with_settings(thumb_w, thumb_h, thumb_cfg.quality);
    thread::spawn(move || {
        thumbnail_worker(thumb_rx, event_tx_thumb, disk_cache);
    });

    // Spawn event polling thread.
    let event_tx_input = event_tx.clone();
    thread::spawn(move || {
        input_worker(event_tx_input);
    });

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

/// Background thread that loads thumbnails using fast_image_resize.
fn thumbnail_worker(
    rx: Receiver<ThumbnailRequest>,
    tx: SyncSender<AppEvent>,
    disk_cache: ThumbnailCache,
) {
    let mut perf = WorkerPerf::new(perf_enabled());
    while let Ok(first_request) = rx.recv() {
        // Drain available work and keep only the newest generation.
        // Also deduplicate by cache index so fast scrolling doesn't waste
        // time decoding thumbnails that were immediately superseded.
        let requests = collect_latest_requests(first_request, &rx);
        perf.record_batch(requests.len());

        for request in requests {
            let decode_started = Instant::now();
            // Load thumbnail (uses fast_image_resize with disk caching).
            match disk_cache.load(&request.source_path) {
                Ok(image) => {
                    perf.record_decode(decode_started.elapsed(), true);
                    let response = ThumbnailResponse {
                        cache_idx: request.cache_idx,
                        image,
                        generation: request.generation,
                    };
                    if !send_thumbnail_ready(&tx, response) {
                        return;
                    }
                }
                Err(e) => {
                    perf.record_decode(decode_started.elapsed(), false);
                    eprintln!(
                        "Thumbnail failed for {}: {}",
                        request.source_path.display(),
                        e
                    );
                }
            }
        }
        perf.maybe_log();
    }
}

fn send_thumbnail_ready(tx: &SyncSender<AppEvent>, response: ThumbnailResponse) -> bool {
    tx.send(AppEvent::ThumbnailReady(response)).is_ok()
}

fn collect_latest_requests(
    first_request: ThumbnailRequest,
    rx: &Receiver<ThumbnailRequest>,
) -> Vec<ThumbnailRequest> {
    let mut latest_generation = first_request.generation;
    let mut ordered_cache_idxs = vec![first_request.cache_idx];
    let mut latest_by_idx: HashMap<usize, ThumbnailRequest> = HashMap::new();
    latest_by_idx.insert(first_request.cache_idx, first_request);

    while let Ok(request) = rx.try_recv() {
        if request.generation > latest_generation {
            latest_generation = request.generation;
            ordered_cache_idxs.clear();
            latest_by_idx.clear();
        }

        if request.generation == latest_generation {
            if !latest_by_idx.contains_key(&request.cache_idx) {
                ordered_cache_idxs.push(request.cache_idx);
            }
            latest_by_idx.insert(request.cache_idx, request);
        }
    }

    let mut ordered = Vec::with_capacity(latest_by_idx.len());
    for cache_idx in ordered_cache_idxs {
        if let Some(request) = latest_by_idx.remove(&cache_idx) {
            ordered.push(request);
        }
    }
    ordered
}

/// Background thread that polls for input events.
fn input_worker(tx: SyncSender<AppEvent>) {
    loop {
        if event::poll(std::time::Duration::from_millis(50)).unwrap_or(false) {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if tx.send(AppEvent::Key(key)).is_err() {
                        break;
                    }
                }
                Ok(Event::Resize(_, _)) => {
                    if tx.send(AppEvent::Resize).is_err() {
                        break;
                    }
                }
                _ => {}
            }
        } else {
            match tx.try_send(AppEvent::Tick) {
                Ok(()) | Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
    }
}

fn coalesce_thumbnail_events(events: Vec<AppEvent>) -> Vec<AppEvent> {
    let mut coalesced = Vec::with_capacity(events.len());
    let mut latest_generation: Option<u64> = None;
    let mut ordered_cache_idxs = Vec::new();
    let mut latest_by_idx: HashMap<usize, ThumbnailResponse> = HashMap::new();

    for event in events {
        match event {
            AppEvent::ThumbnailReady(response) => {
                match latest_generation {
                    None => latest_generation = Some(response.generation),
                    Some(gen) if response.generation > gen => {
                        latest_generation = Some(response.generation);
                        ordered_cache_idxs.clear();
                        latest_by_idx.clear();
                    }
                    Some(gen) if response.generation < gen => continue,
                    Some(_) => {}
                }
                if !latest_by_idx.contains_key(&response.cache_idx) {
                    ordered_cache_idxs.push(response.cache_idx);
                }
                latest_by_idx.insert(response.cache_idx, response);
            }
            other => coalesced.push(other),
        }
    }

    if !latest_by_idx.is_empty() {
        for cache_idx in ordered_cache_idxs {
            if let Some(response) = latest_by_idx.remove(&cache_idx) {
                coalesced.push(AppEvent::ThumbnailReady(response));
            }
        }
    }

    coalesced
}

fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    event_rx: Receiver<AppEvent>,
) -> Result<()> {
    let mut last_theme_check = std::time::Instant::now();
    let mut current_theme_is_light = crate::ui::theme::is_light_theme();
    let mut needs_redraw = true;
    let mut pending_thumbnail_redraw = false;
    let mut last_draw_at = Instant::now();
    let mut perf = RuntimePerf::new(perf_enabled());
    let theme_check_interval =
        std::time::Duration::from_millis(app.config.theme.check_interval_ms.max(100));
    let event_wait_timeout = theme_check_interval.min(std::time::Duration::from_millis(100));

    loop {
        // If thumbnail bursts were throttled, trigger redraw once interval has passed.
        if pending_thumbnail_redraw && last_draw_at.elapsed() >= THUMBNAIL_REDRAW_INTERVAL {
            pending_thumbnail_redraw = false;
            needs_redraw = true;
        }

        // Check for theme change on configured interval and force full redraw.
        if last_theme_check.elapsed() >= theme_check_interval {
            let new_is_light = crate::ui::theme::is_light_theme();
            if new_is_light != current_theme_is_light {
                current_theme_is_light = new_is_light;
                app.ui.theme = crate::ui::theme::frost_theme();
                terminal.clear()?; // Force full terminal redraw.
                needs_redraw = true;
            }
            last_theme_check = std::time::Instant::now();
        }

        // Only redraw when needed (event received or state changed).
        if needs_redraw {
            let draw_started = Instant::now();
            terminal.draw(|f| ui::draw(f, app))?;
            perf.record_draw(draw_started.elapsed());
            last_draw_at = Instant::now();
            needs_redraw = false;
        }

        // Block until event arrives (with timeout for theme checks).
        let wait_timeout = if pending_thumbnail_redraw {
            event_wait_timeout.min(THUMBNAIL_REDRAW_INTERVAL)
        } else {
            event_wait_timeout
        };
        let events: Vec<AppEvent> = match event_rx.recv_timeout(wait_timeout) {
            Ok(event) => {
                let mut events = vec![event];
                while let Ok(e) = event_rx.try_recv() {
                    events.push(e);
                }
                coalesce_thumbnail_events(events)
            }
            Err(_) => continue, // Timeout, check theme and loop.
        };

        let mut redraw_from_events = false;
        let batch_len = events.len();
        let process_started = Instant::now();
        for event in events {
            perf.record_event(&event);
            match event {
                AppEvent::Key(key) => {
                    if key.kind != KeyEventKind::Press {
                        continue;
                    }
                    pending_thumbnail_redraw = false;
                    redraw_from_events = true;

                    // Handle help popup first (blocks other input).
                    if app.ui.show_help {
                        match key.code {
                            KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter => {
                                app.ui.show_help = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle color picker popup.
                    if app.ui.show_color_picker {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('C') => {
                                app.ui.show_color_picker = false;
                            }
                            KeyCode::Char('l') | KeyCode::Right => {
                                app.color_picker_next();
                            }
                            KeyCode::Char('h') | KeyCode::Left => {
                                app.color_picker_prev();
                            }
                            KeyCode::Enter => {
                                app.apply_color_filter();
                            }
                            KeyCode::Char('x') | KeyCode::Backspace => {
                                app.clear_color_filter();
                                app.ui.show_color_picker = false;
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle pairing preview popup.
                    if app.pairing.show_preview {
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('p') => {
                                app.pairing.show_preview = false;
                            }
                            KeyCode::Char('l') | KeyCode::Right | KeyCode::Char('n') => {
                                app.pairing_preview_next();
                            }
                            KeyCode::Char('h') | KeyCode::Left | KeyCode::Char('N') => {
                                app.pairing_preview_prev();
                            }
                            KeyCode::Enter => {
                                if let Err(e) = app.apply_pairing_preview() {
                                    app.ui.status_message = Some(format!("{}", e));
                                }
                            }
                            KeyCode::Char('y') => {
                                app.toggle_pairing_style_mode();
                            }
                            KeyCode::Char(c) if c.is_ascii_digit() => {
                                let idx = if c == '0' {
                                    9
                                } else {
                                    (c as u8 - b'1') as usize
                                };
                                let max = app.pairing_preview_alternatives();
                                if idx < max {
                                    app.pairing.preview_idx = idx;
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Handle command mode (vim-style :).
                    if app.ui.command_mode {
                        match key.code {
                            KeyCode::Esc => {
                                app.exit_command_mode();
                            }
                            KeyCode::Enter => {
                                app.execute_command();
                            }
                            KeyCode::Backspace => {
                                app.command_backspace();
                            }
                            KeyCode::Char(c) => {
                                app.command_input(c);
                            }
                            _ => {}
                        }
                        continue;
                    }

                    // Use configurable keybindings.
                    let kb = &app.config.keybindings;
                    let code = key.code;

                    // Quit (configurable + Esc always works).
                    if kb.matches(code, &kb.quit) || code == KeyCode::Esc {
                        app.ui.should_quit = true;
                    }
                    // Navigation (configurable + arrow keys always work).
                    else if kb.matches(code, &kb.next) || code == KeyCode::Right {
                        app.next_wallpaper();
                    } else if kb.matches(code, &kb.prev) || code == KeyCode::Left {
                        app.prev_wallpaper();
                    }
                    // Screen navigation (configurable).
                    else if kb.matches(code, &kb.next_screen) {
                        app.next_screen();
                    } else if kb.matches(code, &kb.prev_screen) {
                        app.prev_screen();
                    }
                    // Apply wallpaper (configurable).
                    else if kb.matches(code, &kb.apply) {
                        if let Err(e) = app.apply_wallpaper() {
                            app.ui.status_message = Some(format!("{}", e));
                        }
                    }
                    // Random wallpaper (configurable).
                    else if kb.matches(code, &kb.random) {
                        if let Err(e) = app.random_wallpaper() {
                            app.ui.status_message = Some(format!("{}", e));
                        }
                    }
                    // Toggle match mode (configurable).
                    else if kb.matches(code, &kb.toggle_match) {
                        app.toggle_match_mode();
                    }
                    // Toggle resize mode (configurable).
                    else if kb.matches(code, &kb.toggle_resize) {
                        app.toggle_resize_mode();
                    }
                    // Non-configurable keys.
                    else {
                        match code {
                            KeyCode::Char(':') => app.enter_command_mode(),
                            KeyCode::Char('?') => app.toggle_help(),
                            KeyCode::Char('s') => app.toggle_sort_mode(),
                            KeyCode::Char('a') | KeyCode::Char('A') => app.toggle_aspect_sort(),
                            KeyCode::Char('c') => app.toggle_colors(),
                            KeyCode::Char('C') => app.toggle_color_picker(),
                            KeyCode::Char('p') => app.toggle_pairing_preview(),
                            KeyCode::Char('t') => app.cycle_tag_filter(),
                            KeyCode::Char('T') => app.clear_tag_filter(),
                            KeyCode::Char('w') => {
                                if let Err(e) = app.export_pywal() {
                                    app.ui.status_message = Some(format!("pywal: {}", e));
                                }
                            }
                            KeyCode::Char('i') => app.toggle_thumbnail_protocol_mode(),
                            KeyCode::Char('W') => app.toggle_pywal_export(),
                            KeyCode::Char('u') => {
                                // Undo pairing.
                                if let Err(e) = app.do_undo() {
                                    app.ui.status_message = Some(format!("Undo: {}", e));
                                }
                            }
                            KeyCode::Char('R') => {
                                // Rescan wallpaper directory.
                                match app.rescan() {
                                    Ok(msg) => {
                                        app.ui.status_message = Some(format!("Rescan: {}", msg));
                                    }
                                    Err(e) => {
                                        app.ui.status_message = Some(format!("Rescan: {}", e));
                                    }
                                }
                            }
                            _ => {}
                        }
                    }
                }
                AppEvent::ThumbnailReady(response) => {
                    app.handle_thumbnail_ready(response);
                    if last_draw_at.elapsed() >= THUMBNAIL_REDRAW_INTERVAL {
                        pending_thumbnail_redraw = false;
                        redraw_from_events = true;
                    } else {
                        pending_thumbnail_redraw = true;
                    }
                }
                AppEvent::Resize => {
                    app.handle_resize();
                    terminal.clear()?;
                    pending_thumbnail_redraw = false;
                    redraw_from_events = true;
                }
                AppEvent::Tick => {
                    // Check for expired undo window and debounced pairing refresh.
                    if app.tick_undo() || app.update_pairing_suggestions_if_due() {
                        pending_thumbnail_redraw = false;
                        redraw_from_events = true;
                    }
                }
            }
        }
        let process_elapsed = process_started.elapsed();
        perf.record_event_batch(batch_len, process_elapsed);

        needs_redraw |= redraw_from_events;
        perf.maybe_log();
        if app.ui.should_quit {
            return Ok(());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{coalesce_thumbnail_events, collect_latest_requests, send_thumbnail_ready};
    use crate::app::{AppEvent, ThumbnailRequest, ThumbnailResponse};
    use std::sync::mpsc;

    fn request(cache_idx: usize, generation: u64, suffix: &str) -> ThumbnailRequest {
        ThumbnailRequest {
            cache_idx,
            source_path: format!("/tmp/{suffix}.png").into(),
            generation,
        }
    }

    fn response(cache_idx: usize, generation: u64) -> ThumbnailResponse {
        ThumbnailResponse {
            cache_idx,
            image: image::DynamicImage::new_rgba8(1, 1),
            generation,
        }
    }

    #[test]
    fn collect_latest_requests_keeps_newest_generation_only() {
        let (tx, rx) = mpsc::sync_channel(16);
        tx.send(request(0, 1, "old-a")).expect("send old-a");
        tx.send(request(1, 1, "old-b")).expect("send old-b");
        tx.send(request(2, 2, "new-a")).expect("send new-a");
        tx.send(request(3, 2, "new-b")).expect("send new-b");

        let first = rx.recv().expect("recv first");
        let mut batch = collect_latest_requests(first, &rx);
        batch.sort_by_key(|r| r.cache_idx);

        assert_eq!(batch.len(), 2);
        assert!(batch.iter().all(|r| r.generation == 2));
        assert_eq!(batch[0].cache_idx, 2);
        assert_eq!(batch[1].cache_idx, 3);
    }

    #[test]
    fn collect_latest_requests_deduplicates_cache_idx() {
        let (tx, rx) = mpsc::sync_channel(16);
        tx.send(request(7, 5, "first")).expect("send first");
        tx.send(request(7, 5, "second")).expect("send second");
        tx.send(request(8, 5, "other")).expect("send other");

        let first = rx.recv().expect("recv first");
        let mut batch = collect_latest_requests(first, &rx);
        batch.sort_by_key(|r| r.cache_idx);

        assert_eq!(batch.len(), 2);
        assert_eq!(batch[0].cache_idx, 7);
        assert_eq!(batch[0].source_path.to_string_lossy(), "/tmp/second.png");
        assert_eq!(batch[1].cache_idx, 8);
    }

    #[test]
    fn collect_latest_requests_preserves_priority_order() {
        let (tx, rx) = mpsc::sync_channel(16);
        tx.send(request(3, 9, "first")).expect("send first");
        tx.send(request(1, 9, "second")).expect("send second");
        tx.send(request(2, 9, "third")).expect("send third");
        tx.send(request(1, 9, "second-new"))
            .expect("send second-new");

        let first = rx.recv().expect("recv first");
        let batch = collect_latest_requests(first, &rx);
        let ordered_idxs: Vec<usize> = batch.iter().map(|r| r.cache_idx).collect();

        assert_eq!(ordered_idxs, vec![3, 1, 2]);
        assert_eq!(
            batch[1].source_path.to_string_lossy(),
            "/tmp/second-new.png"
        );
    }

    #[test]
    fn send_thumbnail_ready_returns_false_when_receiver_is_gone() {
        let (tx, rx) = mpsc::sync_channel(1);
        drop(rx);
        assert!(!send_thumbnail_ready(&tx, response(42, 9)));
    }

    #[test]
    fn coalesce_thumbnail_events_keeps_latest_generation_and_dedupes_idx() {
        let events = vec![
            AppEvent::ThumbnailReady(response(0, 1)),
            AppEvent::Tick,
            AppEvent::ThumbnailReady(response(1, 1)),
            AppEvent::ThumbnailReady(response(2, 2)),
            AppEvent::ThumbnailReady(response(2, 2)),
            AppEvent::ThumbnailReady(response(3, 2)),
        ];

        let coalesced = coalesce_thumbnail_events(events);
        let mut kept = Vec::new();
        let mut saw_tick = false;

        for event in coalesced {
            match event {
                AppEvent::ThumbnailReady(r) => kept.push((r.cache_idx, r.generation)),
                AppEvent::Tick => saw_tick = true,
                _ => {}
            }
        }

        kept.sort_unstable();
        assert!(saw_tick);
        assert_eq!(kept, vec![(2, 2), (3, 2)]);
    }
}
