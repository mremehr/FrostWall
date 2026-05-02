use super::layout::{build_carousel_plan, CarouselPlan};
use super::THUMBNAIL_GAP;
use crate::app::App;
use crate::ui::layout::{center_vertically, fit_aspect, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH};
use crate::ui::theme::FrostTheme;
use crate::utils::display_path_name;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

fn render_empty_state(f: &mut Frame, area: Rect, theme: &FrostTheme) {
    let empty = Paragraph::new("No matching wallpapers")
        .style(Style::default().fg(theme.fg_muted))
        .alignment(Alignment::Center);
    f.render_widget(empty, center_vertically(area, 1));
}

fn wallpaper_label(app: &App, cache_idx: usize) -> (String, bool) {
    app.cache
        .wallpapers
        .get(cache_idx)
        .map(|wallpaper| {
            (
                display_path_name(&wallpaper.path).into_owned(),
                app.is_pairing_suggestion(&wallpaper.path),
            )
        })
        .unwrap_or_else(|| ("?".to_string(), false))
}

fn truncate_label(label: &str, max_chars: usize) -> String {
    if max_chars == 0 {
        return String::new();
    }
    if label.chars().count() <= max_chars {
        return label.to_string();
    }

    let truncated: String = label.chars().take(max_chars.saturating_sub(1)).collect();
    format!("{truncated}…")
}

fn request_thumbnails_for_plan(app: &mut App, plan: &CarouselPlan) {
    let preload = app.config.thumbnails.preload_count;
    let preload_start = plan.start.saturating_sub(preload);
    let preload_end = (plan.end + preload).min(plan.total);

    let mut visible_indices: Vec<usize> = (plan.start..plan.end).collect();
    visible_indices.sort_by_key(|idx| idx.abs_diff(plan.clamped_idx));
    for idx in visible_indices {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        app.request_thumbnail(cache_idx);
    }
    for idx in preload_start..plan.start {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        app.request_thumbnail(cache_idx);
    }
    for idx in plan.end..preload_end {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        app.request_thumbnail(cache_idx);
    }
}

/// Pick a braille spinner glyph based on wall-clock time.
///
/// The runtime forces a redraw every `SPINNER_TICK` while any thumbnail is
/// loading, so the value cycles ~8 frames per second without us tracking
/// frame counts in app state.
fn spinner_glyph() -> &'static str {
    const FRAMES: &[&str] = &[
        "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
    ];
    let elapsed_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis())
        .unwrap_or(0);
    let idx = ((elapsed_ms / 120) % FRAMES.len() as u128) as usize;
    FRAMES[idx]
}

fn fade_level(slot_idx: usize, center_slot: usize, visible: usize) -> usize {
    if visible <= 1 {
        return 0;
    }

    match slot_idx.abs_diff(center_slot) {
        0 => 0,
        1 => 1,
        _ => 2,
    }
}

pub(in crate::ui::layout) fn draw_carousel_single(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &FrostTheme,
) {
    if app.selection.filtered_wallpapers.is_empty() {
        render_empty_state(f, area, theme);
        return;
    }

    let wallpaper_idx = app
        .selection
        .wallpaper_idx
        .min(app.selection.filtered_wallpapers.len().saturating_sub(1));
    let cache_idx = app.selection.filtered_wallpapers[wallpaper_idx];
    let filename = wallpaper_label(app, cache_idx).0;

    app.request_thumbnail(cache_idx);

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

    let max_thumb_w = panel_inner.width.saturating_sub(4);
    let max_thumb_h = panel_inner.height.saturating_sub(4);
    let (thumb_w, thumb_h) = fit_aspect(max_thumb_w, max_thumb_h.saturating_sub(1), 16, 9);
    if thumb_w == 0 || thumb_h == 0 {
        return;
    }
    let thumb_x = panel_inner.x + (panel_inner.width.saturating_sub(thumb_w)) / 2;
    let thumb_y = panel_inner.y + (panel_inner.height.saturating_sub(thumb_h + 1)) / 2;
    let thumb_area = Rect::new(thumb_x, thumb_y, thumb_w, thumb_h);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_highlight))
        .style(Style::default().bg(theme.bg_medium));

    let inner = block.inner(thumb_area);
    f.render_widget(block, thumb_area);

    if let Some(protocol) = app.get_thumbnail(cache_idx) {
        let image = StatefulImage::new(None);
        f.render_stateful_widget(image, inner, protocol);
    } else {
        let label = Paragraph::new(filename)
            .style(Style::default().fg(theme.fg_secondary))
            .alignment(Alignment::Center);
        f.render_widget(label, center_vertically(inner, 1));
    }

    if thumb_area.bottom() < panel_inner.y + panel_inner.height {
        let indicator_area = Rect::new(thumb_x, thumb_area.bottom(), thumb_w, 1);
        let indicator = Paragraph::new("▲ Selected")
            .style(Style::default().fg(theme.accent_highlight))
            .alignment(Alignment::Center);
        f.render_widget(indicator, indicator_area);
    }
}

pub(in crate::ui::layout) fn draw_carousel(
    f: &mut Frame,
    app: &mut App,
    area: Rect,
    theme: &FrostTheme,
) {
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

    let can_go_left = app.selection.wallpaper_idx > 0;
    let left_arrow = Paragraph::new(if can_go_left { "❮" } else { " " })
        .style(Style::default().fg(if can_go_left {
            theme.accent_primary
        } else {
            theme.fg_muted
        }))
        .alignment(Alignment::Center);
    f.render_widget(left_arrow, center_vertically(chunks[0], 1));

    let can_go_right =
        app.selection.wallpaper_idx < app.selection.filtered_wallpapers.len().saturating_sub(1);
    let right_arrow = Paragraph::new(if can_go_right { "❯" } else { " " })
        .style(Style::default().fg(if can_go_right {
            theme.accent_primary
        } else {
            theme.fg_muted
        }))
        .alignment(Alignment::Center);
    f.render_widget(right_arrow, center_vertically(chunks[2], 1));

    draw_thumbnails(f, app, chunks[1], theme);
}

pub(super) fn draw_thumbnails(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    let Some(plan) = build_carousel_plan(app, area) else {
        render_empty_state(f, area, theme);
        return;
    };

    request_thumbnails_for_plan(app, &plan);

    let mut cursor_x = plan.start_x;
    for (slot_idx, idx) in (plan.start..plan.end).enumerate() {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        let is_selected = idx == plan.clamped_idx;
        let slot_width = plan
            .slot_widths
            .get(slot_idx)
            .copied()
            .unwrap_or(THUMBNAIL_WIDTH);
        let slot_content_height = plan
            .slot_heights
            .get(slot_idx)
            .copied()
            .unwrap_or(THUMBNAIL_HEIGHT);
        let slot_height = slot_content_height.saturating_add(2);
        let y_offset = plan
            .max_slot_content_height
            .saturating_sub(slot_content_height)
            / 2;
        let thumb_y = plan.thumb_row_y.saturating_add(y_offset);
        let fade_level = fade_level(slot_idx, plan.center_slot, plan.visible_count());
        let next_cursor = cursor_x
            .saturating_add(slot_width)
            .saturating_add(THUMBNAIL_GAP);

        let (filename, is_suggestion) = wallpaper_label(app, cache_idx);
        let is_loading = app.is_loading(cache_idx);
        let thumb_x = cursor_x;

        if thumb_x + slot_width > area.x + area.width
            || thumb_y + slot_height > area.y + area.height
        {
            cursor_x = next_cursor;
            continue;
        }

        let thumb_area = Rect::new(thumb_x, thumb_y, slot_width, slot_height);
        let border_color = if is_selected {
            theme.accent_highlight
        } else if is_suggestion {
            theme.success
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
            } else if fade_level >= 2 {
                theme.bg_dark
            } else {
                theme.bg_medium
            }));

        let inner = block.inner(thumb_area);
        f.render_widget(block, thumb_area);

        if let Some(protocol) = app.get_thumbnail(cache_idx) {
            let image = StatefulImage::new(None).resize(Resize::Fit(None));
            f.render_stateful_widget(image, inner, protocol);
        } else if is_loading {
            let loading = Paragraph::new(spinner_glyph())
                .style(Style::default().fg(theme.accent_primary))
                .alignment(Alignment::Center);
            f.render_widget(loading, center_vertically(inner, 1));
        } else {
            let label = Paragraph::new(truncate_label(&filename, inner.width as usize))
                .style(Style::default().fg(theme.fg_secondary))
                .alignment(Alignment::Center);
            f.render_widget(label, center_vertically(inner, 1));
        }

        if thumb_area.bottom() < area.y + area.height {
            let indicator_area = Rect::new(thumb_x, thumb_area.bottom(), slot_width, 1);

            if is_selected {
                let indicator = Paragraph::new("▲")
                    .style(Style::default().fg(theme.accent_highlight))
                    .alignment(Alignment::Center);
                f.render_widget(indicator, indicator_area);
            } else if is_suggestion {
                let indicator = Paragraph::new("★ paired")
                    .style(Style::default().fg(theme.success))
                    .alignment(Alignment::Center);
                f.render_widget(indicator, indicator_area);
            }
        }

        cursor_x = next_cursor;
    }
}
