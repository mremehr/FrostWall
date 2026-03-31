use super::input::handle_key_event;
use super::workers::coalesce_thumbnail_events;
use super::THUMBNAIL_REDRAW_INTERVAL;
use crate::app::perf::{perf_enabled, RuntimePerf};
use crate::app::{App, AppEvent};
use crate::ui;
use anyhow::Result;
use crossterm::event::KeyEventKind;
use ratatui::Terminal;
use std::sync::mpsc::Receiver;
use std::time::{Duration, Instant};

fn recv_coalesced_events(
    event_rx: &Receiver<AppEvent>,
    wait_timeout: Duration,
) -> Option<Vec<AppEvent>> {
    let mut events = vec![event_rx.recv_timeout(wait_timeout).ok()?];
    while let Ok(event) = event_rx.try_recv() {
        events.push(event);
    }
    Some(coalesce_thumbnail_events(events))
}

fn check_theme_changes(
    app: &mut App,
    last_check: &mut Instant,
    is_light: &mut bool,
    interval: Duration,
) -> bool {
    if last_check.elapsed() < interval {
        return false;
    }

    *last_check = Instant::now();
    let new_is_light = crate::ui::theme::is_light_theme();
    if new_is_light != *is_light {
        *is_light = new_is_light;
        app.ui.theme = crate::ui::theme::frost_theme();
        return true;
    }

    false
}

fn handle_app_event<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    event: AppEvent,
    last_draw_at: Instant,
    pending_thumbnail_redraw: &mut bool,
) -> Result<bool> {
    match event {
        AppEvent::Key(key) => {
            if key.kind != KeyEventKind::Press {
                return Ok(false);
            }
            *pending_thumbnail_redraw = false;
            handle_key_event(app, key);
            Ok(true)
        }
        AppEvent::ThumbnailReady(response) => {
            app.handle_thumbnail_ready(response);
            if last_draw_at.elapsed() >= THUMBNAIL_REDRAW_INTERVAL {
                *pending_thumbnail_redraw = false;
                Ok(true)
            } else {
                *pending_thumbnail_redraw = true;
                Ok(false)
            }
        }
        AppEvent::Resize => {
            app.handle_resize();
            terminal.clear()?;
            *pending_thumbnail_redraw = false;
            Ok(true)
        }
        AppEvent::Tick => {
            if app.tick_undo() || app.update_pairing_suggestions_if_due() {
                *pending_thumbnail_redraw = false;
                Ok(true)
            } else {
                Ok(false)
            }
        }
    }
}

pub(super) fn run_app<B: ratatui::backend::Backend>(
    terminal: &mut Terminal<B>,
    app: &mut App,
    event_rx: Receiver<AppEvent>,
) -> Result<()> {
    let mut last_theme_check = Instant::now();
    let mut current_theme_is_light = crate::ui::theme::is_light_theme();
    let mut needs_redraw = true;
    let mut pending_thumbnail_redraw = false;
    let mut last_draw_at = Instant::now();
    let mut perf = RuntimePerf::new(perf_enabled());
    let theme_check_interval = Duration::from_millis(app.config.theme.check_interval_ms.max(100));
    let event_wait_timeout = theme_check_interval.min(Duration::from_millis(100));

    loop {
        if pending_thumbnail_redraw && last_draw_at.elapsed() >= THUMBNAIL_REDRAW_INTERVAL {
            pending_thumbnail_redraw = false;
            needs_redraw = true;
        }

        if check_theme_changes(
            app,
            &mut last_theme_check,
            &mut current_theme_is_light,
            theme_check_interval,
        ) {
            terminal.clear()?;
            needs_redraw = true;
        }

        if needs_redraw {
            let draw_started = Instant::now();
            terminal.draw(|frame| ui::draw(frame, app))?;
            perf.record_draw(draw_started.elapsed());
            last_draw_at = Instant::now();
            needs_redraw = false;
        }

        let wait_timeout = if pending_thumbnail_redraw {
            event_wait_timeout.min(THUMBNAIL_REDRAW_INTERVAL)
        } else {
            event_wait_timeout
        };
        let Some(events) = recv_coalesced_events(&event_rx, wait_timeout) else {
            continue;
        };

        let batch_len = events.len();
        let process_started = Instant::now();
        let mut redraw_from_events = false;
        for event in events {
            perf.record_event(&event);
            redraw_from_events |= handle_app_event(
                terminal,
                app,
                event,
                last_draw_at,
                &mut pending_thumbnail_redraw,
            )?;
        }
        perf.record_event_batch(batch_len, process_started.elapsed());

        needs_redraw |= redraw_from_events;
        perf.maybe_log();

        if app.ui.should_quit {
            break;
        }
    }

    Ok(())
}
