use super::{center_vertically, fit_aspect, THUMBNAIL_HEIGHT, THUMBNAIL_WIDTH};
use crate::app::App;
use crate::ui::theme::FrostTheme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Clear, Paragraph},
    Frame,
};
use ratatui_image::StatefulImage;

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

    // Calculate visible range centered on selection
    let total = app.selection.filtered_wallpapers.len();
    let grid_columns = app.config.thumbnails.grid_columns;
    let visible = grid_columns.min(total);
    let half = visible / 2;

    // Clamp wallpaper_idx to valid range (defensive against stale index)
    let clamped_idx = app.selection.wallpaper_idx.min(total.saturating_sub(1));

    let start = if clamped_idx <= half {
        0
    } else if clamped_idx >= total.saturating_sub(half + 1) {
        total.saturating_sub(visible)
    } else {
        clamped_idx - half
    };

    let end = (start + visible).min(total);

    // Calculate thumbnail positions
    let thumb_total_width = THUMBNAIL_WIDTH + 2; // +2 for spacing
    let total_thumbs_width = (visible as u16) * thumb_total_width;
    let start_x = area.x + (area.width.saturating_sub(total_thumbs_width)) / 2;

    // Center vertically
    let thumb_y = area.y + (area.height.saturating_sub(THUMBNAIL_HEIGHT + 2)) / 2;

    // Preload extra thumbnails ahead/behind for smooth scrolling.
    let preload = app.config.thumbnails.preload_count;
    let preload_start = start.saturating_sub(preload);
    let preload_end = (end + preload).min(total);

    // Request visible thumbnails first (highest priority).
    for idx in start..end {
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

    for (i, idx) in (start..end).enumerate() {
        let cache_idx = app.selection.filtered_wallpapers[idx];
        let is_selected = idx == clamped_idx;

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

        let thumb_x = start_x + (i as u16) * thumb_total_width;

        // Bounds check - skip if outside visible area
        if thumb_x + THUMBNAIL_WIDTH > area.x + area.width {
            continue;
        }
        if thumb_y + THUMBNAIL_HEIGHT + 2 > area.y + area.height {
            continue;
        }

        let thumb_area = Rect::new(thumb_x, thumb_y, THUMBNAIL_WIDTH, THUMBNAIL_HEIGHT + 2);

        // Draw thumbnail frame - green for suggestions, highlight for selected
        let border_color = if is_selected {
            theme.accent_highlight
        } else if is_suggestion {
            theme.success // Green for pairing suggestions
        } else {
            theme.border
        };

        let border_style = if is_suggestion && !is_selected {
            Style::default()
                .fg(border_color)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(border_color)
        };

        // Clear previous image artifacts (Kitty protocol caches images)
        f.render_widget(Clear, thumb_area);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(border_style)
            .style(Style::default().bg(theme.bg_medium));

        let inner = block.inner(thumb_area);
        f.render_widget(block, thumb_area);

        // Try to render image if available
        if let Some(protocol) = app.get_thumbnail(cache_idx) {
            let image = StatefulImage::new(None);
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
            let indicator_area = Rect::new(thumb_x, thumb_area.bottom(), THUMBNAIL_WIDTH, 1);

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
    }
}
