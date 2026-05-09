use super::{
    DEFAULT_TERMINAL_CELL_ASPECT, LANDSCAPE_RATIO, MAX_CAROUSEL_VISIBLE, MAX_SELECTED_SLOT_WIDTH,
    MAX_SLOT_WIDTH, MAX_TERMINAL_CELL_ASPECT, MIN_SLOT_WIDTH, MIN_THUMB_CONTENT_HEIGHT,
    MIN_TERMINAL_CELL_ASPECT, THUMBNAIL_GAP,
};
use crate::app::App;
use crate::screen::AspectCategory;
use crate::thumbnail::effective_thumbnail_bounds;
use crate::ui::layout::{THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH};
use ratatui::layout::Rect;

const NON_SELECTED_MIN_WIDTH: u16 = 16;

/// Natural slot width for a given aspect ratio. Selection-independent on
/// purpose: stable side widths are what allow `ratatui-image` to skip
/// expensive re-encoding when the user scrolls, since `needs_resize` returns
/// `Some(rect)` whenever the rendered cell-rect changes.
fn slot_width_for_ratio(ratio: f32) -> u16 {
    let safe_ratio = if ratio.is_finite() && ratio > 0.0 {
        ratio
    } else {
        LANDSCAPE_RATIO
    };
    let mut factor = (safe_ratio / LANDSCAPE_RATIO).clamp(0.78, 1.70);
    if safe_ratio >= 2.0 {
        factor *= 1.22;
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
    let max_width_cells = (fit_w / cell_w).floor().max(1.0) as u16;
    let max_height_cells = (fit_h / cell_h).floor().max(1.0) as u16;
    (max_width_cells, max_height_cells)
}

fn content_height_for_slot(slot_width: u16, ratio: f32, max_height: u16, cell_aspect: f32) -> u16 {
    if slot_width == 0 || max_height == 0 {
        return 0;
    }

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
pub(super) struct SlotSpec {
    pub(super) ratio: f32,
    pub(super) max_width: u16,
    pub(super) max_height: u16,
}

pub(super) struct CarouselPlan {
    pub(super) total: usize,
    pub(super) clamped_idx: usize,
    pub(super) start: usize,
    pub(super) end: usize,
    pub(super) center_slot: usize,
    pub(super) slot_widths: Vec<u16>,
    pub(super) slot_heights: Vec<u16>,
    pub(super) start_x: u16,
    pub(super) thumb_row_y: u16,
    pub(super) max_slot_content_height: u16,
}

impl CarouselPlan {
    pub(super) fn visible_count(&self) -> usize {
        self.end.saturating_sub(self.start)
    }
}

/// Lay out slot widths so non-selected slots keep their natural ratio-derived
/// width and the selected slot absorbs all remaining slack. This is the key
/// step that keeps scrolling smooth: as long as side widths don't change, the
/// underlying `StatefulProtocol` skips its `resize_encode` step (which on
/// Kitty/Sixel is a full image resize + base64 transmit per thumbnail).
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

    let mut widths: Vec<u16> = base_widths
        .iter()
        .zip(slot_max_widths.iter())
        .map(|(width, cap)| (*width).min((*cap).max(1)).max(1))
        .collect();

    let selected_cap = slot_max_widths
        .get(selected_slot)
        .copied()
        .unwrap_or(1)
        .max(selected_min_width)
        .max(1);
    let selected_min = selected_min_width.max(1).min(selected_cap);

    // Iteratively shrink the widest non-selected slot until both the selected
    // minimum and the total budget fit. The selected slot is then assigned the
    // exact remainder, so its width is fully determined by the side widths and
    // never overshoots its cap.
    loop {
        let non_sel_sum: u16 = widths
            .iter()
            .enumerate()
            .filter(|(idx, _)| *idx != selected_slot)
            .map(|(_, width)| *width)
            .sum();
        let remainder = max_slots_width.saturating_sub(non_sel_sum);
        let selected_width = remainder.clamp(selected_min, selected_cap);

        if non_sel_sum.saturating_add(selected_width) <= max_slots_width {
            if let Some(slot) = widths.get_mut(selected_slot) {
                *slot = selected_width;
            }
            return widths;
        }

        let shrink_target = widths
            .iter()
            .enumerate()
            .filter(|(idx, width)| *idx != selected_slot && **width > NON_SELECTED_MIN_WIDTH)
            .max_by_key(|(_, width)| *width)
            .map(|(idx, _)| idx);

        match shrink_target {
            Some(idx) => widths[idx] = widths[idx].saturating_sub(1),
            None => {
                // Non-selected slots are at the floor — the selected slot must
                // absorb the deficit even if it dips below `selected_min`.
                if let Some(slot) = widths.get_mut(selected_slot) {
                    let new_value = max_slots_width.saturating_sub(non_sel_sum).max(1);
                    *slot = new_value.min(selected_cap);
                }
                return widths;
            }
        }
    }
}

pub(super) fn build_carousel_plan(app: &App, area: Rect) -> Option<CarouselPlan> {
    if app.selection.filtered_wallpapers.is_empty() {
        return None;
    }

    let total = app.selection.filtered_wallpapers.len();
    let clamped_idx = app.selection.wallpaper_idx.min(total.saturating_sub(1));
    let min_per_slot = (MIN_SLOT_WIDTH + THUMBNAIL_GAP) as usize;
    let max_by_width = (area.width as usize + THUMBNAIL_GAP as usize) / min_per_slot;
    let visible = max_by_width.min(total).clamp(1, MAX_CAROUSEL_VISIBLE);

    let start = centered_window_start(total, clamped_idx, visible);
    let end = (start + visible).min(total);

    let max_content_height = area
        .height
        .saturating_sub(3)
        .clamp(THUMBNAIL_HEIGHT / 2, THUMBNAIL_HEIGHT * 5);
    let cell_aspect = terminal_cell_aspect(app);
    let target_area_cells = (THUMBNAIL_HEIGHT as f32) * (THUMBNAIL_WIDTH as f32);
    let slot_specs: Vec<SlotSpec> = (start..end)
        .map(|idx| {
            let cache_idx = app.selection.filtered_wallpapers[idx];
            let ratio = app
                .cache
                .wallpapers
                .get(cache_idx)
                .map(|wallpaper| {
                    if wallpaper.width > 0 && wallpaper.height > 0 {
                        wallpaper.width as f32 / wallpaper.height as f32
                    } else {
                        nominal_aspect_ratio(wallpaper.aspect_category)
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
    let selected_eq_h = ((target_area_cells / (selected_ratio.max(0.1) * cell_aspect))
        .sqrt()
        .clamp(
            MIN_THUMB_CONTENT_HEIGHT as f32,
            max_content_height as f32 * 2.0,
        )) as u16;
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
    let coupled_slot_max_widths: Vec<u16> = slot_max_widths
        .iter()
        .enumerate()
        .map(|(idx, max_width)| {
            let spec = slot_specs.get(idx).copied().unwrap_or(SlotSpec {
                ratio: LANDSCAPE_RATIO,
                max_width: MAX_SLOT_WIDTH,
                max_height: max_content_height,
            });
            let cap_h = slot_height_cap(idx, idx == selected_slot);
            let max_by_height = ((cap_h as f32) * spec.ratio * cell_aspect).round() as u16;
            (*max_width).min(max_by_height.max(1)).max(1)
        })
        .collect();
    let selected_min_width = selected_min_width.min(
        coupled_slot_max_widths
            .get(selected_slot)
            .copied()
            .unwrap_or(selected_min_width),
    );

    let base_slot_widths: Vec<u16> = slot_specs
        .iter()
        .enumerate()
        .map(|(offset, spec)| {
            let cap = coupled_slot_max_widths
                .get(offset)
                .copied()
                .unwrap_or(MAX_SLOT_WIDTH);
            slot_width_for_ratio(spec.ratio).min(cap).max(1)
        })
        .collect();
    let slot_widths = fit_slot_widths(
        &base_slot_widths,
        &coupled_slot_max_widths,
        area.width,
        selected_slot,
        selected_min_width,
    );
    let slot_heights: Vec<u16> = slot_widths
        .iter()
        .zip(slot_specs.iter())
        .enumerate()
        .map(|(idx, (width, spec))| {
            let slot_cap = slot_height_cap(idx, idx == selected_slot);
            content_height_for_slot(*width, spec.ratio, slot_cap, cell_aspect)
        })
        .collect();

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
    let row_total_height = max_slot_content_height.saturating_add(3);
    let thumb_row_y = area.y + (area.height.saturating_sub(row_total_height)) / 2;

    Some(CarouselPlan {
        total,
        clamped_idx,
        start,
        end,
        center_slot: visible / 2,
        slot_widths,
        slot_heights,
        start_x,
        thumb_row_y,
        max_slot_content_height,
    })
}

#[cfg(test)]
mod tests {
    use super::{centered_window_start, fit_slot_widths, slot_width_for_ratio};

    #[test]
    fn centered_window_clamps_to_left_edge() {
        assert_eq!(centered_window_start(10, 1, 5), 0);
    }

    #[test]
    fn centered_window_clamps_to_right_edge() {
        assert_eq!(centered_window_start(10, 9, 5), 5);
    }

    #[test]
    fn centered_window_centers_when_room_exists() {
        assert_eq!(centered_window_start(10, 5, 5), 3);
    }

    #[test]
    fn fit_slot_widths_respects_selected_minimum() {
        let widths = fit_slot_widths(&[40, 50, 40], &[40, 60, 40], 120, 1, 50);
        assert!(widths[1] >= 50);
        assert!(widths.iter().copied().sum::<u16>() <= 120);
    }

    #[test]
    fn fit_slot_widths_only_expands_selected_slot_when_underfilled() {
        let widths = fit_slot_widths(&[30, 30, 30], &[35, 80, 35], 140, 1, 50);
        assert_eq!(widths[0], 30);
        assert_eq!(widths[2], 30);
        assert!(widths[1] >= 50);
    }

    /// The dominant lag during scroll comes from `ratatui-image` re-encoding any
    /// thumbnail whose rect changes. To keep that work to a minimum we require
    /// the widths of slots that are NOT the (old or new) selected position to
    /// be byte-for-byte identical when selection moves within a stable visible
    /// window. Only two slots may change per scroll step.
    #[test]
    fn fit_slot_widths_keeps_side_slots_stable_when_selection_moves() {
        let base = [50, 50, 50, 50, 50];
        let caps = [120, 120, 120, 120, 120];
        let area = 320;
        let widths_a = fit_slot_widths(&base, &caps, area, 1, 60);
        let widths_b = fit_slot_widths(&base, &caps, area, 2, 60);
        let widths_c = fit_slot_widths(&base, &caps, area, 3, 60);

        // Slot 0 is non-selected in A, B, and C — must be identical.
        assert_eq!(widths_a[0], widths_b[0]);
        assert_eq!(widths_b[0], widths_c[0]);

        // Slot 4 same.
        assert_eq!(widths_a[4], widths_b[4]);
        assert_eq!(widths_b[4], widths_c[4]);

        // When moving A → B, only slots 1 and 2 may change width.
        for idx in [0, 3, 4] {
            assert_eq!(
                widths_a[idx], widths_b[idx],
                "slot {idx} width changed across A→B (was {}, now {})",
                widths_a[idx], widths_b[idx]
            );
        }
    }

    /// Side slots must depend ONLY on the wallpaper's aspect ratio, never on
    /// which sibling is selected. Otherwise selection moves cascade through
    /// every slot's width.
    #[test]
    fn slot_width_for_ratio_is_selection_independent() {
        // Sanity: same ratio, different positions, same width.
        let landscape = slot_width_for_ratio(16.0 / 9.0);
        let landscape_again = slot_width_for_ratio(16.0 / 9.0);
        assert_eq!(landscape, landscape_again);

        // Reasonable spread across categories.
        let portrait = slot_width_for_ratio(9.0 / 16.0);
        let ultrawide = slot_width_for_ratio(21.0 / 9.0);
        assert!(portrait < landscape);
        assert!(landscape < ultrawide);
    }

    /// When base widths are too wide to fit while still meeting the selected
    /// minimum, non-selected slots shrink — but the selected slot remains at
    /// least at its minimum so the focused image stays prominent.
    #[test]
    fn fit_slot_widths_shrinks_non_selected_to_make_room_for_selected() {
        let base = [80, 80, 80];
        let caps = [120, 120, 120];
        let area = 200;
        let widths = fit_slot_widths(&base, &caps, area, 1, 70);
        assert!(widths[1] >= 70, "selected min not respected: {:?}", widths);
        let total: u16 = widths.iter().copied().sum::<u16>()
            + super::THUMBNAIL_GAP * (widths.len() as u16 - 1);
        assert!(total <= area, "exceeded area: {:?} sum {}", widths, total);
    }
}
