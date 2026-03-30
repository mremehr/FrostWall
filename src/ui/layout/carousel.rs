use super::{center_vertically, fit_aspect, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH};
use crate::app::App;
use crate::screen::AspectCategory;
use crate::thumbnail::effective_thumbnail_bounds;
use crate::ui::theme::FrostTheme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

const THUMBNAIL_GAP: u16 = 2;
const DEFAULT_TERMINAL_CELL_ASPECT: f32 = 2.0;
const MIN_TERMINAL_CELL_ASPECT: f32 = 1.2;
const MAX_TERMINAL_CELL_ASPECT: f32 = 3.0;
const MIN_THUMB_CONTENT_HEIGHT: u16 = 8;
const LANDSCAPE_RATIO: f32 = 16.0 / 9.0;
const MIN_SLOT_WIDTH: u16 = 24;
const MAX_CAROUSEL_VISIBLE: usize = 13; // ~338 terminal columns needed at MIN_SLOT_WIDTH
const MAX_SLOT_WIDTH: u16 = 280;
const MAX_SELECTED_SLOT_WIDTH: u16 = 360;
const SELECTED_WIDTH_BOOST: f32 = 1.25;
const SELECTED_ULTRAWIDE_BOOST: f32 = 1.12;

fn slot_width_for_ratio(ratio: f32, is_selected: bool) -> u16 {
    let safe_ratio = if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        LANDSCAPE_RATIO
    };
    let mut factor = (safe_ratio / LANDSCAPE_RATIO).clamp(0.78, 1.70);
    if safe_ratio >= 2.0 {
        // Keep ultrawide prominent without overpowering the row.
        factor *= 1.22;
    }
    if is_selected {
        factor *= SELECTED_WIDTH_BOOST;
        if safe_ratio >= 2.0 {
            factor *= SELECTED_ULTRAWIDE_BOOST;
        } else if safe_ratio <= 1.1 {
            // Make selected square/portrait tiles visually pop, not just ultrawide.
            factor *= 1.24;
            if safe_ratio < 0.85 {
                factor *= 1.14;
            }
        }
    }
    ((THUMBNAIL_WIDTH as f32) * factor)
        .round()
        .clamp(MIN_SLOT_WIDTH as f32, MAX_SLOT_WIDTH as f32) as u16
}

fn nominal_aspect_ratio(aspect: AspectCategory) -> f32 {
    match aspect {
        AspectCategory::Ultrawide => 21.0 / 9.0,
        AspectCategory::Landscape => 16.0 / 9.0,
        AspectCategory::Square => 1.0,
        AspectCategory::Portrait => 9.0 / 16.0,
    }
}

fn terminal_cell_aspect(app: &App) -> f32 {
    app.thumbnails
        .image_picker
        .as_ref()
        .and_then(|picker| {
            let (cell_w, cell_h) = picker.font_size;
            if cell_w > 0 && cell_h > 0 {
                Some(cell_h as f32 / cell_w as f32)
            } else {
                None
            }
        })
        .unwrap_or(DEFAULT_TERMINAL_CELL_ASPECT)
        .clamp(MIN_TERMINAL_CELL_ASPECT, MAX_TERMINAL_CELL_ASPECT)
}

fn terminal_cell_size(app: &App) -> (u16, u16) {
    app.thumbnails
        .image_picker
        .as_ref()
        .map(|picker| picker.font_size)
        .unwrap_or((8, 16))
}

fn thumbnail_cell_limits(app: &App, ratio: f32) -> (u16, u16) {
    let safe_ratio = if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        LANDSCAPE_RATIO
    };
    let (max_thumb_w, max_thumb_h) =
        effective_thumbnail_bounds(app.config.thumbnails.width, app.config.thumbnails.height);
    let max_thumb_w = max_thumb_w.max(1);
    let max_thumb_h = max_thumb_h.max(1);

    let max_ratio = max_thumb_w as f32 / max_thumb_h as f32;
    let (fit_w, fit_h) = if max_ratio >= safe_ratio {
        (
            (max_thumb_h as f32 * safe_ratio).round().max(1.0),
            max_thumb_h as f32,
        )
    } else {
        (
            max_thumb_w as f32,
            (max_thumb_w as f32 / safe_ratio).round().max(1.0),
        )
    };

    let (cell_w, cell_h) = terminal_cell_size(app);
    let cell_w = cell_w.max(1) as f32;
    let cell_h = cell_h.max(1) as f32;
    let max_w_cells = (fit_w / cell_w).floor().max(1.0) as u16;
    let max_h_cells = (fit_h / cell_h).floor().max(1.0) as u16;
    (max_w_cells, max_h_cells)
}

fn content_height_for_slot(slot_width: u16, ratio: f32, max_height: u16, cell_aspect: f32) -> u16 {
    if slot_width == 0 || max_height == 0 {
        return 0;
    }
    // Ratatui image sizing happens in terminal cells (not square pixels).
    // Convert nominal image ratio into cell-space height to avoid large letterboxing.
    let safe_ratio = if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        LANDSCAPE_RATIO
    };
    let estimated = ((slot_width as f32) / (safe_ratio * cell_aspect)).round() as u16;
    estimated
        .clamp(MIN_THUMB_CONTENT_HEIGHT.min(max_height), max_height)
        .max(1)
}

fn centered_window_start(total: usize, selected: usize, count: usize) -> usize {
    if total <= count {
        return 0;
    }

    let half = count / 2;
    if selected <= half {
        0
    } else if selected >= total.saturating_sub(half + 1) {
        total.saturating_sub(count)
    } else {
        selected - half
    }
}

#[derive(Clone, Copy)]
struct SlotSpec {
    ratio: f32,
    max_width: u16,
    max_height: u16,
}

fn fit_slot_widths(
    base_widths: &[u16],
    slot_max_widths: &[u16],
    area_width: u16,
    selected_slot: usize,
    selected_min_width: u16,
) -> Vec<u16> {
    if base_widths.is_empty() {
        return Vec::new();
    }
    if base_widths.len() != slot_max_widths.len() {
        return base_widths.to_vec();
    }

    let gaps = THUMBNAIL_GAP.saturating_mul(base_widths.len().saturating_sub(1) as u16);
    let max_slots_width = area_width.saturating_sub(gaps);
    if max_slots_width == 0 {
        return vec![1; base_widths.len()];
    }

    let mut widths = base_widths.to_vec();
    let mut sum: u16 = widths.iter().copied().sum();
    let selected_cap = slot_max_widths
        .get(selected_slot)
        .copied()
        .unwrap_or(1)
        .max(selected_min_width)
        .max(1);
    if sum < max_slots_width && sum > 0 {
        // Expand proportionally first to better use row width and avoid dead space.
        let scale = (max_slots_width as f32 / sum as f32).min(1.7);
        for (idx, width) in widths.iter_mut().enumerate() {
            let cap = slot_max_widths.get(idx).copied().unwrap_or(1).max(1);
            *width = ((*width as f32) * scale).round().clamp(1.0, cap as f32) as u16;
        }
        sum = widths.iter().copied().sum();
    }
    if sum < max_slots_width {
        // Distribute remaining space while keeping selected tile slightly dominant.
        let mut selected_streak: u8 = 0;
        while sum < max_slots_width {
            let selected_candidate = if selected_streak < 3 {
                widths
                    .get(selected_slot)
                    .copied()
                    .filter(|w| *w < selected_cap)
                    .map(|_| selected_slot)
            } else {
                None
            };
            let non_selected_candidate = widths
                .iter()
                .enumerate()
                .filter(|(idx, w)| {
                    *idx != selected_slot && **w < slot_max_widths.get(*idx).copied().unwrap_or(1)
                })
                .max_by_key(|(_, w)| *w)
                .map(|(idx, _)| idx);
            let fallback_candidate = widths
                .get(selected_slot)
                .copied()
                .filter(|w| *w < selected_cap)
                .map(|_| selected_slot);

            let Some(idx) = selected_candidate
                .or(non_selected_candidate)
                .or(fallback_candidate)
            else {
                break;
            };
            widths[idx] = widths[idx].saturating_add(1);
            sum = sum.saturating_add(1);
            if idx == selected_slot {
                selected_streak = selected_streak.saturating_add(1);
            } else {
                selected_streak = 0;
            }
        }
        return widths;
    } else if sum <= max_slots_width {
        return widths;
    }

    // Keep non-dominant slots readable and preserve selected slot prominence.
    let min_width = 16;
    while sum > max_slots_width {
        let candidate_non_selected = widths
            .iter()
            .enumerate()
            .filter(|(idx, w)| *idx != selected_slot && **w > min_width)
            .max_by_key(|(_, w)| *w)
            .map(|(idx, _)| idx);

        let candidate_selected = widths
            .get(selected_slot)
            .copied()
            .filter(|w| *w > selected_min_width.max(1))
            .map(|_| selected_slot);

        let candidate_fallback = widths
            .iter()
            .enumerate()
            .filter(|(_, w)| **w > 1)
            .max_by_key(|(_, w)| *w)
            .map(|(idx, _)| idx);

        let candidate = candidate_non_selected
            .or(candidate_selected)
            .or(candidate_fallback);

        let Some(idx) = candidate else {
            break;
        };
        widths[idx] = widths[idx].saturating_sub(1);
        sum = sum.saturating_sub(1);
    }

    widths
}

pub(super) fn draw_carousel_single(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    if app.selection.filtered_wallpapers.is_empty() {
        let empty = Paragraph::new("No matching wallpapers")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        let centered = center_vertically(area, 1);
        f.render_widget(empty, centered);
        return;
    }

    let wallpaper_idx = app
        .selection
        .wallpaper_idx
        .min(app.selection.filtered_wallpapers.len().saturating_sub(1));
    let cache_idx = app.selection.filtered_wallpapers[wallpaper_idx];

    // Get wallpaper info
    let filename = app
        .cache
        .wallpapers
        .get(cache_idx)
        .map(|wp| {
            wp.path
                .file_stem()
                .and_then(|n| n.to_str())
                .unwrap_or("?")
                .to_string()
        })
        .unwrap_or("?".to_string());

    // Request thumbnail
    app.request_thumbnail(cache_idx);

    // Full selected panel so left/right sides have consistent visual structure.
    let panel = Block::default()
        .title(" Selected ")
        .title_style(
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_highlight))
        .style(Style::default().bg(theme.bg_dark));
    let panel_inner = panel.inner(area);
    f.render_widget(panel, area);

    // Dynamic thumbnail sizing: fit to inner panel and keep a cinematic aspect ratio.
    let max_thumb_w = panel_inner.width.saturating_sub(4);
    let max_thumb_h = panel_inner.height.saturating_sub(4);
    let (thumb_w, thumb_h) = fit_aspect(max_thumb_w, max_thumb_h.saturating_sub(1), 16, 9);
    if thumb_w == 0 || thumb_h == 0 {
        return;
    }
    let thumb_x = panel_inner.x + (panel_inner.width.saturating_sub(thumb_w)) / 2;
    let thumb_y = panel_inner.y + (panel_inner.height.saturating_sub(thumb_h + 1)) / 2;
    let thumb_area = Rect::new(thumb_x, thumb_y, thumb_w, thumb_h);

    // Draw frame
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_highlight))
        .style(Style::default().bg(theme.bg_medium));

    let inner = block.inner(thumb_area);
    f.render_widget(block, thumb_area);

    // Render image
    if let Some(protocol) = app.get_thumbnail(cache_idx) {
        let image = StatefulImage::new(None);
        f.render_stateful_widget(image, inner, protocol);
    } else {
        // Fallback: show filename
        let label = Paragraph::new(filename)
            .style(Style::default().fg(theme.fg_secondary))
            .alignment(Alignment::Center);
        f.render_widget(label, center_vertically(inner, 1));
    }

    // Selection indicator below
    if thumb_area.bottom() < panel_inner.y + panel_inner.height {
        let indicator_area = Rect::new(thumb_x, thumb_area.bottom(), thumb_w, 1);
        let indicator = Paragraph::new("▲ Selected")
            .style(Style::default().fg(theme.accent_highlight))
            .alignment(Alignment::Center);
        f.render_widget(indicator, indicator_area);
    }
}

pub(super) fn draw_carousel(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    // Horizontal layout: left arrow, thumbnails, right arrow
    let arrow_width = 3;
    let thumbnails_area_width = area.width.saturating_sub(arrow_width * 2);

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(arrow_width),
            Constraint::Min(thumbnails_area_width),
            Constraint::Length(arrow_width),
        ])
        .split(area);

    // Left arrow
    let can_go_left = app.selection.wallpaper_idx > 0;
    let left_arrow = Paragraph::new(if can_go_left { "❮" } else { " " })
        .style(Style::default().fg(if can_go_left {
            theme.accent_primary
        } else {
            theme.fg_muted
        }))
        .alignment(Alignment::Center);

    // Center vertically
    let left_area = center_vertically(chunks[0], 1);
    f.render_widget(left_arrow, left_area);

    // Right arrow
    let can_go_right =
        app.selection.wallpaper_idx < app.selection.filtered_wallpapers.len().saturating_sub(1);
    let right_arrow = Paragraph::new(if can_go_right { "❯" } else { " " })
        .style(Style::default().fg(if can_go_right {
            theme.accent_primary
        } else {
            theme.fg_muted
        }))
        .alignment(Alignment::Center);

    let right_area = center_vertically(chunks[2], 1);
    f.render_widget(right_arrow, right_area);

    // Thumbnails area
    draw_thumbnails(f, app, chunks[1], theme);
}

pub(super) fn draw_thumbnails(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    if app.selection.filtered_wallpapers.is_empty() {
        let empty = Paragraph::new("No matching wallpapers")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        let centered = center_vertically(area, 1);
        f.render_widget(empty, centered);
        return;
    }

    // Always target 5 thumbnails when available (or fewer if total < 5),
    // and keep selected wallpaper centered when possible.
    let total = app.selection.filtered_wallpapers.len();

    // Clamp wallpaper_idx to valid range (defensive against stale index)
    let clamped_idx = app.selection.wallpaper_idx.min(total.saturating_sub(1));

    // How many slots fit: N*MIN + (N-1)*GAP ≤ width → N ≤ (width+GAP)/(MIN+GAP)
    let min_per_slot = (MIN_SLOT_WIDTH + THUMBNAIL_GAP) as usize; // always > 0
    let max_by_width = (area.width as usize + THUMBNAIL_GAP as usize) / min_per_slot;
    let visible = max_by_width.min(total).clamp(1, MAX_CAROUSEL_VISIBLE);

    let start = centered_window_start(total, clamped_idx, visible);
    let end = (start + visible).min(total);

    // Calculate variable slot widths and content heights.
    let max_content_height = area
        .height
        .saturating_sub(3)
        .clamp(THUMBNAIL_HEIGHT / 2, THUMBNAIL_HEIGHT * 5);
    let cell_aspect = terminal_cell_aspect(app);
    // Equal-area height for selected slot: portrait gets more height, ultrawide less,
    // so all formats occupy roughly the same visual area when selected.
    // target_area = THUMBNAIL_HEIGHT * THUMBNAIL_WIDTH (baseline area in cells²)
    let target_area_cells = (THUMBNAIL_HEIGHT as f32) * (THUMBNAIL_WIDTH as f32);
    let slot_specs: Vec<SlotSpec> = (start..end)
        .map(|idx| {
            let cache_idx = app.selection.filtered_wallpapers[idx];
            let ratio = app
                .cache
                .wallpapers
                .get(cache_idx)
                .map(|wp| {
                    if wp.width > 0 && wp.height > 0 {
                        wp.width as f32 / wp.height as f32
                    } else {
                        nominal_aspect_ratio(wp.aspect_category)
                    }
                })
                .unwrap_or(LANDSCAPE_RATIO);
            let (max_width, max_height) = thumbnail_cell_limits(app, ratio);
            SlotSpec {
                ratio,
                max_width,
                max_height,
            }
        })
        .collect();
    let selected_slot = clamped_idx
        .saturating_sub(start)
        .min(visible.saturating_sub(1));
    let selected_ratio = slot_specs
        .get(selected_slot)
        .map(|spec| spec.ratio)
        .unwrap_or(LANDSCAPE_RATIO);
    // Equal-area height for the selected slot: solve h such that (h * ratio * cell_aspect) * h = target_area
    // → h = sqrt(target_area / (ratio * cell_aspect))
    let selected_eq_h = ((target_area_cells / (selected_ratio.max(0.1) * cell_aspect))
        .sqrt()
        .clamp(
            MIN_THUMB_CONTENT_HEIGHT as f32,
            max_content_height as f32 * 2.0,
        )) as u16;
    // Derive equal-area width and use as selected_min_width floor
    let selected_eq_w = (selected_eq_h as f32 * selected_ratio * cell_aspect).round() as u16;
    let selected_min_width = if selected_ratio >= 2.0 {
        THUMBNAIL_WIDTH + (THUMBNAIL_WIDTH / 2) + 6
    } else {
        selected_eq_w.max(THUMBNAIL_WIDTH + 8)
    };
    let slot_height_cap = |slot_idx: usize, selected: bool| {
        let max_height = slot_specs
            .get(slot_idx)
            .map(|spec| spec.max_height)
            .unwrap_or(max_content_height)
            .max(1);
        if selected {
            selected_eq_h.min(max_height)
        } else {
            max_content_height.min(max_height)
        }
    };
    let slot_max_widths: Vec<u16> = slot_specs
        .iter()
        .enumerate()
        .map(|(offset, spec)| {
            let idx = start + offset;
            let is_selected = idx == clamped_idx;
            let ratio_cap = if is_selected && spec.ratio >= 2.0 {
                MAX_SELECTED_SLOT_WIDTH
            } else if is_selected {
                MAX_SLOT_WIDTH + (THUMBNAIL_WIDTH / 2)
            } else {
                MAX_SLOT_WIDTH
            };
            spec.max_width.min(ratio_cap).max(1)
        })
        .collect();
    // Keep width and height coupled so portrait/square cards do not become overly wide when
    // height is capped by layout constraints.
    // For the selected slot we use the equal-area height (selected_eq_h) so all formats
    // get roughly the same visual area regardless of aspect ratio.
    let coupled_slot_max_widths: Vec<u16> = slot_max_widths
        .iter()
        .enumerate()
        .map(|(i, max_w)| {
            let spec = slot_specs.get(i).copied().unwrap_or(SlotSpec {
                ratio: LANDSCAPE_RATIO,
                max_width: MAX_SLOT_WIDTH,
                max_height: max_content_height,
            });
            // Equal-area cap: portrait gets more height, ultrawide less.
            let cap_h = slot_height_cap(i, i == selected_slot);
            let max_by_height = ((cap_h as f32) * spec.ratio * cell_aspect).round() as u16;
            (*max_w).min(max_by_height.max(1)).max(1)
        })
        .collect();
    let selected_min_width = selected_min_width.min(
        coupled_slot_max_widths
            .get(selected_slot)
            .copied()
            .unwrap_or(selected_min_width),
    );

    let mut base_slot_widths: Vec<u16> = slot_specs
        .iter()
        .enumerate()
        .map(|(offset, spec)| {
            let idx = start + offset;
            let cap = coupled_slot_max_widths
                .get(offset)
                .copied()
                .unwrap_or(MAX_SLOT_WIDTH);
            slot_width_for_ratio(spec.ratio, idx == clamped_idx)
                .min(cap)
                .max(1)
        })
        .collect();
    // Edge pseudo-fade: shrink far sides first, but never the selected slot.
    if visible > 1 {
        let center = visible / 2;
        for (i, width) in base_slot_widths.iter_mut().enumerate() {
            if i == selected_slot {
                continue;
            }
            let distance = i.abs_diff(center);
            let scale = match distance {
                0 => 1.0,
                1 => 0.90, // adjacent: subtly smaller
                _ => 0.78, // outer edges: moderately smaller
            };
            let cap = slot_max_widths
                .get(i)
                .copied()
                .unwrap_or(MAX_SLOT_WIDTH)
                .max(1);
            let cap = coupled_slot_max_widths
                .get(i)
                .copied()
                .unwrap_or(cap)
                .max(1);
            *width = ((*width as f32) * scale).round().clamp(1.0, cap as f32) as u16;
        }
    }
    let slot_widths = fit_slot_widths(
        &base_slot_widths,
        &coupled_slot_max_widths,
        area.width,
        selected_slot,
        selected_min_width,
    );
    let mut slot_heights: Vec<u16> = slot_widths
        .iter()
        .zip(slot_specs.iter())
        .enumerate()
        .map(|(i, (w, spec))| {
            let slot_cap = slot_height_cap(i, i == selected_slot);
            content_height_for_slot(*w, spec.ratio, slot_cap, cell_aspect)
        })
        .collect();

    // Pseudo-fade: side slots are a bit smaller so center stays visually dominant.
    if visible > 1 {
        let center = visible / 2;
        for (i, (h, spec)) in slot_heights.iter_mut().zip(slot_specs.iter()).enumerate() {
            let distance = i.abs_diff(center);
            if i == selected_slot {
                continue;
            }
            let scale = match distance {
                0 => 1.0,
                1 => 0.90, // adjacent: subtly shorter
                _ => {
                    if spec.ratio >= 2.0 {
                        0.80
                    } else {
                        0.75
                    }
                } // outer edges
            };
            let scaled = ((*h as f32) * scale).round() as u16;
            let slot_cap = slot_height_cap(i, false);
            *h = scaled
                .clamp(MIN_THUMB_CONTENT_HEIGHT.min(slot_cap), slot_cap)
                .max(1);
        }
    }

    let total_thumbs_width: u16 =
        slot_widths.iter().copied().sum::<u16>().saturating_add(
            THUMBNAIL_GAP.saturating_mul(slot_widths.len().saturating_sub(1) as u16),
        );
    let start_x = area.x + (area.width.saturating_sub(total_thumbs_width)) / 2;

    let max_slot_content_height = slot_heights
        .iter()
        .copied()
        .max()
        .unwrap_or(THUMBNAIL_HEIGHT);
    let row_total_height = max_slot_content_height.saturating_add(3); // frame + indicator
    let thumb_row_y = area.y + (area.height.saturating_sub(row_total_height)) / 2;
    let center_slot = visible / 2;

    // Preload extra thumbnails ahead/behind for smooth scrolling.
    let preload = app.config.thumbnails.preload_count;
    let preload_start = start.saturating_sub(preload);
    let preload_end = (end + preload).min(total);

    // Request visible thumbnails first (highest priority).
    let mut visible_indices: Vec<usize> = (start..end).collect();
    visible_indices.sort_by_key(|idx| idx.abs_diff(clamped_idx));
    for idx in visible_indices {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        app.request_thumbnail(cache_idx);
    }
    // Then request preload range behind and ahead.
    for idx in preload_start..start {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        app.request_thumbnail(cache_idx);
    }
    for idx in end..preload_end {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        app.request_thumbnail(cache_idx);
    }

    let mut cursor_x = start_x;
    for (i, idx) in (start..end).enumerate() {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        let is_selected = idx == clamped_idx;
        let slot_width = slot_widths.get(i).copied().unwrap_or(THUMBNAIL_WIDTH);
        let slot_content_height = slot_heights.get(i).copied().unwrap_or(THUMBNAIL_HEIGHT);
        let slot_height = slot_content_height.saturating_add(2);
        let y_offset = max_slot_content_height.saturating_sub(slot_content_height) / 2;
        let thumb_y = thumb_row_y.saturating_add(y_offset);
        let distance_from_center = i.abs_diff(center_slot);
        let fade_level = if visible > 1 {
            match distance_from_center {
                0 => 0, // center
                1 => 1, // near edges
                _ => 2, // outer edges (1 and 5)
            }
        } else {
            0
        };
        let next_cursor = cursor_x
            .saturating_add(slot_width)
            .saturating_add(THUMBNAIL_GAP);

        // Get wallpaper info before mutable borrow
        let (filename, is_suggestion) = app
            .cache
            .wallpapers
            .get(cache_idx)
            .map(|wp| {
                let name = wp
                    .path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                let suggested = app.is_pairing_suggestion(&wp.path);
                (name, suggested)
            })
            .unwrap_or(("?".to_string(), false));

        let is_loading = app.is_loading(cache_idx);
        let thumb_x = cursor_x;

        // Bounds check - skip if outside visible area
        if thumb_x + slot_width > area.x + area.width {
            cursor_x = next_cursor;
            continue;
        }
        if thumb_y + slot_height > area.y + area.height {
            cursor_x = next_cursor;
            continue;
        }

        let thumb_area = Rect::new(thumb_x, thumb_y, slot_width, slot_height);

        // Draw thumbnail frame - green for suggestions, highlight for selected
        let border_color = if is_selected {
            theme.accent_highlight
        } else if is_suggestion {
            theme.success // Green for pairing suggestions
        } else if fade_level >= 2 {
            theme.fg_muted
        } else {
            theme.border
        };

        let border_style = if is_suggestion && !is_selected {
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD)
        } else if fade_level >= 1 && !is_selected {
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(border_color)
        };

        // Clear previous image artifacts (Kitty protocol caches images)
        f.render_widget(Clear, thumb_area);

        let block = Block::default()
            .borders(if is_selected {
                Borders::ALL
            } else {
                Borders::NONE
            })
            .border_style(border_style)
            .style(Style::default().bg(if is_selected {
                theme.bg_medium
            } else {
                match fade_level {
                    2 => theme.bg_dark,
                    _ => theme.bg_medium,
                }
            }));

        let inner = block.inner(thumb_area);
        f.render_widget(block, thumb_area);

        // Try to render image if available
        if let Some(protocol) = app.get_thumbnail(cache_idx) {
            // All thumbnails use Fit — no cropping, full image visible.
            let resize = Resize::Fit(None);
            let image = StatefulImage::new(None).resize(resize);
            f.render_stateful_widget(image, inner, protocol);
        } else if is_loading {
            // Show loading indicator
            let loading = Paragraph::new("...")
                .style(Style::default().fg(theme.accent_primary))
                .alignment(Alignment::Center);
            let loading_area = center_vertically(inner, 1);
            f.render_widget(loading, loading_area);
        } else {
            // Fallback: show filename
            let max_chars = inner.width as usize;
            let display = if max_chars == 0 {
                String::new()
            } else if filename.chars().count() <= max_chars {
                filename.clone()
            } else {
                // Safe truncation using char boundaries
                let truncated: String =
                    filename.chars().take(max_chars.saturating_sub(1)).collect();
                format!("{}…", truncated)
            };

            let label = Paragraph::new(display)
                .style(Style::default().fg(theme.fg_secondary))
                .alignment(Alignment::Center);

            let label_area = center_vertically(inner, 1);
            f.render_widget(label, label_area);
        }

        // Indicators below thumbnail (with bounds check)
        if thumb_area.bottom() < area.y + area.height {
            let indicator_area = Rect::new(thumb_x, thumb_area.bottom(), slot_width, 1);

            if is_selected {
                // Selection indicator
                let indicator = Paragraph::new("▲")
                    .style(Style::default().fg(theme.accent_highlight))
                    .alignment(Alignment::Center);
                f.render_widget(indicator, indicator_area);
            } else if is_suggestion {
                // Pairing suggestion indicator
                let indicator = Paragraph::new("★ paired")
                    .style(Style::default().fg(theme.success))
                    .alignment(Alignment::Center);
                f.render_widget(indicator, indicator_area);
            }
        }

        cursor_x = next_cursor;
    }
}
