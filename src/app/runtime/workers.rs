use crate::app::perf::{perf_enabled, WorkerPerf};
use crate::app::{
    AnalysisFailure, AnalysisRequest, AnalysisResponse, AppEvent, ThumbnailFailure,
    ThumbnailRequest, ThumbnailResponse,
};
use crate::thumbnail::ThumbnailCache;
use crate::wallpaper::{extract_palette_from_image, extract_palette_from_path};
use crossterm::event::{self, Event};
use rayon::prelude::*;
use rayon::ThreadPool;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, SyncSender, TrySendError};
use std::time::Duration;

const INPUT_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Background thread that loads thumbnails using fast_image_resize.
pub(super) fn thumbnail_worker(
    rx: Receiver<ThumbnailRequest>,
    tx: SyncSender<AppEvent>,
    disk_cache: ThumbnailCache,
) {
    let mut perf = WorkerPerf::new(perf_enabled());
    let pool = build_worker_pool(4, 1);
    while let Ok(first_request) = rx.recv() {
        // Drain available work and keep only the newest generation.
        // Also deduplicate by cache index so fast scrolling doesn't waste
        // time decoding thumbnails that were immediately superseded.
        let requests = collect_latest_requests(first_request, &rx);
        perf.record_batch(requests.len());

        let results = pool.install(|| {
            requests
                .into_par_iter()
                .map(|request| {
                    let result = disk_cache.load(&request.source_path);
                    (request, result)
                })
                .collect::<Vec<_>>()
        });

        for (request, result) in results {
            match result {
                Ok(image) => {
                    if let Some(generation) = request.analysis_generation {
                        match extract_palette_from_image(&image) {
                            Ok((colors, color_weights)) => {
                                let response = AnalysisResponse {
                                    cache_idx: request.cache_idx,
                                    colors,
                                    color_weights,
                                    generation,
                                };
                                if tx.send(AppEvent::AnalysisReady(response)).is_err() {
                                    return;
                                }
                            }
                            Err(error) => {
                                eprintln!(
                                    "Color analysis failed for {}: {}",
                                    request.source_path.display(),
                                    error
                                );
                                let failure = AnalysisFailure {
                                    cache_idx: request.cache_idx,
                                    generation,
                                };
                                if tx.send(AppEvent::AnalysisFailed(failure)).is_err() {
                                    return;
                                }
                            }
                        }
                    }

                    let response = ThumbnailResponse {
                        cache_idx: request.cache_idx,
                        image,
                        generation: request.thumbnail_generation,
                    };
                    if !send_thumbnail_ready(&tx, response) {
                        return;
                    }
                }
                Err(error) => {
                    eprintln!(
                        "Thumbnail failed for {}: {}",
                        request.source_path.display(),
                        error
                    );
                    if let Some(generation) = request.analysis_generation {
                        let failure = AnalysisFailure {
                            cache_idx: request.cache_idx,
                            generation,
                        };
                        if tx.send(AppEvent::AnalysisFailed(failure)).is_err() {
                            return;
                        }
                    }
                    let failure = ThumbnailFailure {
                        cache_idx: request.cache_idx,
                        generation: request.thumbnail_generation,
                    };
                    if tx.send(AppEvent::ThumbnailFailed(failure)).is_err() {
                        return;
                    }
                }
            }
        }
        perf.maybe_log();
    }
}

pub(super) fn analysis_worker(rx: Receiver<AnalysisRequest>, tx: SyncSender<AppEvent>) {
    let mut perf = WorkerPerf::new(perf_enabled());
    let pool = build_worker_pool(8, 2);

    while let Ok(first_request) = rx.recv() {
        let requests = collect_latest_analysis_requests(first_request, &rx);
        perf.record_batch(requests.len());

        let results = pool.install(|| {
            requests
                .into_par_iter()
                .map(|request| {
                    let result = extract_palette_from_path(&request.source_path);
                    (request, result)
                })
                .collect::<Vec<_>>()
        });

        for (request, result) in results {
            match result {
                Ok((colors, color_weights)) => {
                    let response = AnalysisResponse {
                        cache_idx: request.cache_idx,
                        colors,
                        color_weights,
                        generation: request.generation,
                    };
                    if tx.send(AppEvent::AnalysisReady(response)).is_err() {
                        return;
                    }
                }
                Err(error) => {
                    eprintln!(
                        "Color analysis failed for {}: {}",
                        request.source_path.display(),
                        error
                    );
                    let failure = AnalysisFailure {
                        cache_idx: request.cache_idx,
                        generation: request.generation,
                    };
                    if tx.send(AppEvent::AnalysisFailed(failure)).is_err() {
                        return;
                    }
                }
            }
        }

        perf.maybe_log();
    }
}

fn send_thumbnail_ready(tx: &SyncSender<AppEvent>, response: ThumbnailResponse) -> bool {
    tx.send(AppEvent::ThumbnailReady(response)).is_ok()
}

fn latest_generation_by_cache_idx<T>(
    items: impl IntoIterator<Item = T>,
    cache_idx_of: impl Fn(&T) -> usize,
    generation_of: impl Fn(&T) -> u64,
) -> Vec<T> {
    let mut latest_generation = None;
    let mut ordered_cache_idxs = Vec::new();
    let mut latest_by_idx: HashMap<usize, T> = HashMap::new();

    for item in items {
        let generation = generation_of(&item);
        match latest_generation {
            None => latest_generation = Some(generation),
            Some(current) if generation > current => {
                latest_generation = Some(generation);
                ordered_cache_idxs.clear();
                latest_by_idx.clear();
            }
            Some(current) if generation < current => continue,
            Some(_) => {}
        }

        let cache_idx = cache_idx_of(&item);
        if !latest_by_idx.contains_key(&cache_idx) {
            ordered_cache_idxs.push(cache_idx);
        }
        latest_by_idx.insert(cache_idx, item);
    }

    let mut ordered = Vec::with_capacity(latest_by_idx.len());
    for cache_idx in ordered_cache_idxs {
        if let Some(item) = latest_by_idx.remove(&cache_idx) {
            ordered.push(item);
        }
    }
    ordered
}

fn collect_latest_requests(
    first_request: ThumbnailRequest,
    rx: &Receiver<ThumbnailRequest>,
) -> Vec<ThumbnailRequest> {
    let mut requests = vec![first_request];
    while let Ok(request) = rx.try_recv() {
        requests.push(request);
    }

    latest_generation_by_cache_idx(
        requests,
        |request| request.cache_idx,
        |request| request.thumbnail_generation,
    )
}

fn collect_latest_analysis_requests(
    first_request: AnalysisRequest,
    rx: &Receiver<AnalysisRequest>,
) -> Vec<AnalysisRequest> {
    let mut requests = vec![first_request];
    while let Ok(request) = rx.try_recv() {
        requests.push(request);
    }

    latest_generation_by_cache_idx(
        requests,
        |request| request.cache_idx,
        |request| request.generation,
    )
}

fn build_worker_pool(max_threads: usize, reserve_cores: usize) -> ThreadPool {
    let available = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1);
    let threads = available
        .saturating_sub(reserve_cores)
        .clamp(1, max_threads.max(1));

    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build()
        .expect("worker thread pool should build")
}

/// Background thread that polls for input events.
pub(super) fn input_worker(tx: SyncSender<AppEvent>) {
    loop {
        if event::poll(INPUT_POLL_INTERVAL).unwrap_or(false) {
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

pub(super) fn coalesce_thumbnail_events(events: Vec<AppEvent>) -> Vec<AppEvent> {
    if !events.iter().any(|event| {
        matches!(
            event,
            AppEvent::ThumbnailReady(_)
                | AppEvent::ThumbnailFailed(_)
                | AppEvent::AnalysisReady(_)
                | AppEvent::AnalysisFailed(_)
        )
    }) {
        return events;
    }

    let mut coalesced = Vec::with_capacity(events.len());
    let mut thumbnail_events = Vec::new();
    let mut thumbnail_failures = Vec::new();
    let mut analysis_events = Vec::new();
    let mut analysis_failures = Vec::new();

    for event in events {
        match event {
            AppEvent::ThumbnailReady(response) => thumbnail_events.push(response),
            AppEvent::ThumbnailFailed(failure) => thumbnail_failures.push(failure),
            AppEvent::AnalysisReady(response) => analysis_events.push(response),
            AppEvent::AnalysisFailed(failure) => analysis_failures.push(failure),
            other => coalesced.push(other),
        }
    }

    coalesced.extend(
        latest_generation_by_cache_idx(
            thumbnail_events,
            |response| response.cache_idx,
            |response| response.generation,
        )
        .into_iter()
        .map(AppEvent::ThumbnailReady),
    );
    coalesced.extend(
        latest_generation_by_cache_idx(
            thumbnail_failures,
            |failure| failure.cache_idx,
            |failure| failure.generation,
        )
        .into_iter()
        .map(AppEvent::ThumbnailFailed),
    );
    coalesced.extend(
        latest_generation_by_cache_idx(
            analysis_events,
            |response| response.cache_idx,
            |response| response.generation,
        )
        .into_iter()
        .map(AppEvent::AnalysisReady),
    );
    coalesced.extend(
        latest_generation_by_cache_idx(
            analysis_failures,
            |failure| failure.cache_idx,
            |failure| failure.generation,
        )
        .into_iter()
        .map(AppEvent::AnalysisFailed),
    );

    coalesced
}

#[cfg(test)]
mod tests {
    use super::{coalesce_thumbnail_events, latest_generation_by_cache_idx};
    use crate::app::{AnalysisResponse, AppEvent, ThumbnailResponse};

    #[derive(Debug, PartialEq, Eq)]
    struct GenerationItem {
        cache_idx: usize,
        generation: u64,
        label: &'static str,
    }

    fn thumbnail_ready(cache_idx: usize, generation: u64) -> AppEvent {
        AppEvent::ThumbnailReady(ThumbnailResponse {
            cache_idx,
            image: image::DynamicImage::new_rgba8(1, 1),
            generation,
        })
    }

    fn analysis_ready(cache_idx: usize, generation: u64) -> AppEvent {
        AppEvent::AnalysisReady(AnalysisResponse {
            cache_idx,
            colors: vec!["#112233".to_string()],
            color_weights: vec![1.0],
            generation,
        })
    }

    #[test]
    fn latest_generation_only_keeps_newest_batch_per_cache_index() {
        let items = vec![
            GenerationItem {
                cache_idx: 1,
                generation: 1,
                label: "old-a",
            },
            GenerationItem {
                cache_idx: 2,
                generation: 1,
                label: "old-b",
            },
            GenerationItem {
                cache_idx: 1,
                generation: 2,
                label: "new-a",
            },
            GenerationItem {
                cache_idx: 3,
                generation: 2,
                label: "new-c",
            },
            GenerationItem {
                cache_idx: 1,
                generation: 2,
                label: "new-a-replaced",
            },
        ];

        let latest =
            latest_generation_by_cache_idx(items, |item| item.cache_idx, |item| item.generation);

        assert_eq!(
            latest,
            vec![
                GenerationItem {
                    cache_idx: 1,
                    generation: 2,
                    label: "new-a-replaced",
                },
                GenerationItem {
                    cache_idx: 3,
                    generation: 2,
                    label: "new-c",
                },
            ]
        );
    }

    #[test]
    fn coalesce_thumbnail_events_preserves_non_thumbnail_order() {
        let events = vec![
            AppEvent::Tick,
            thumbnail_ready(1, 1),
            AppEvent::Resize,
            thumbnail_ready(1, 2),
            thumbnail_ready(2, 2),
        ];

        let coalesced = coalesce_thumbnail_events(events);

        assert!(matches!(coalesced.first(), Some(AppEvent::Tick)));
        assert!(matches!(coalesced.get(1), Some(AppEvent::Resize)));

        let thumbnails: Vec<_> = coalesced
            .into_iter()
            .filter_map(|event| match event {
                AppEvent::ThumbnailReady(response) => {
                    Some((response.cache_idx, response.generation))
                }
                _ => None,
            })
            .collect();
        assert_eq!(thumbnails, vec![(1, 2), (2, 2)]);
    }

    #[test]
    fn coalesce_thumbnail_events_deduplicates_analysis_ready() {
        let events = vec![
            analysis_ready(1, 1),
            analysis_ready(1, 2),
            AppEvent::Tick,
            analysis_ready(2, 2),
        ];

        let coalesced = coalesce_thumbnail_events(events);
        let analyses: Vec<_> = coalesced
            .into_iter()
            .filter_map(|event| match event {
                AppEvent::AnalysisReady(response) => {
                    Some((response.cache_idx, response.generation))
                }
                _ => None,
            })
            .collect();

        assert_eq!(analyses, vec![(1, 2), (2, 2)]);
    }
}
