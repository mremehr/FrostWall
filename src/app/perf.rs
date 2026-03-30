use super::AppEvent;
use std::time::{Duration, Instant};

pub(super) const PERF_LOG_INTERVAL: Duration = Duration::from_secs(3);

pub(super) fn perf_enabled() -> bool {
    std::env::var("FROSTWALL_PERF")
        .map(|v| {
            let v = v.trim().to_ascii_lowercase();
            matches!(v.as_str(), "1" | "true" | "yes" | "on")
        })
        .unwrap_or(false)
}

#[derive(Debug)]
pub(super) struct RuntimePerf {
    pub enabled: bool,
    pub window_start: Instant,
    pub draw_count: u64,
    pub draw_total: Duration,
    pub draw_max: Duration,
    pub event_batches: u64,
    pub event_total: u64,
    pub event_process_total: Duration,
    pub event_process_max: Duration,
    pub key_events: u64,
    pub thumbnail_events: u64,
    pub resize_events: u64,
    pub tick_events: u64,
}

impl RuntimePerf {
    pub fn new(enabled: bool) -> Self {
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

    pub fn record_draw(&mut self, elapsed: Duration) {
        if !self.enabled {
            return;
        }
        self.draw_count = self.draw_count.saturating_add(1);
        self.draw_total += elapsed;
        self.draw_max = self.draw_max.max(elapsed);
    }

    pub fn record_event_batch(&mut self, events: usize, process_elapsed: Duration) {
        if !self.enabled {
            return;
        }
        self.event_batches = self.event_batches.saturating_add(1);
        self.event_total = self.event_total.saturating_add(events as u64);
        self.event_process_total += process_elapsed;
        self.event_process_max = self.event_process_max.max(process_elapsed);
    }

    pub fn record_event(&mut self, event: &AppEvent) {
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

    pub fn maybe_log(&mut self) {
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

        *self = Self::new(self.enabled);
    }
}

#[derive(Debug)]
pub(super) struct WorkerPerf {
    pub enabled: bool,
    pub window_start: Instant,
    pub batches: u64,
    pub requests: u64,
}

impl WorkerPerf {
    pub fn new(enabled: bool) -> Self {
        Self {
            enabled,
            window_start: Instant::now(),
            batches: 0,
            requests: 0,
        }
    }

    pub fn record_batch(&mut self, size: usize) {
        if !self.enabled {
            return;
        }
        self.batches = self.batches.saturating_add(1);
        self.requests = self.requests.saturating_add(size as u64);
    }

    pub fn maybe_log(&mut self) {
        if !self.enabled || self.window_start.elapsed() < PERF_LOG_INTERVAL {
            return;
        }

        eprintln!(
            "[perf][thumb-worker] batches={} requests={}",
            self.batches, self.requests,
        );

        *self = Self::new(self.enabled);
    }
}
