use crate::app::App;
use crate::ui::theme::FrostTheme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::Style,
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

mod carousel;
mod header;
mod pairing;
mod popups;

use carousel::{draw_carousel, draw_carousel_single};
use header::{draw_error, draw_footer, draw_header};
use pairing::draw_pairing_panel;
use popups::{draw_carousel_placeholder, draw_color_picker, draw_help_popup, draw_undo_popup};

const THUMBNAIL_WIDTH: u16 = 48;
const THUMBNAIL_HEIGHT: u16 = 28;

pub fn draw(f: &mut Frame, app: &mut App) {
    let theme = app.ui.theme;
    let area = f.area();

    // Check if a popup is showing (need to skip image rendering)
    // ratatui-image renders directly to terminal, bypassing widget z-order
    // Note: show_pairing_preview renders thumbnails separately, so don't block carousel
    let popup_active = app.ui.show_help
        || app.ui.show_color_picker
        || app.pairing.history.can_undo()
        || app.ui.command_mode;

    // Main container with frost border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.border_focused))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Vertical layout: header, carousel, (optional error), (optional colors), footer
    let has_error = app.ui.status_message.is_some();
    let constraints = build_layout_constraints(app.ui.show_colors, has_error);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(inner);

    let mut chunk_idx = 0;

    draw_header(f, app, chunks[chunk_idx], &theme);
    chunk_idx += 1;

    if has_error {
        draw_error(f, app, chunks[chunk_idx], &theme);
        chunk_idx += 1;
    }

    // Only draw carousel with images if no popup is active
    // (ratatui-image renders directly to terminal, bypassing widget z-order)
    if popup_active {
        draw_carousel_placeholder(f, chunks[chunk_idx], &theme);
    } else if app.pairing.show_preview {
        // Always show the split when preview mode is on — even if the current
        // style mode produced zero matches. Otherwise we'd silently fall back
        // to the carousel while the input handler keeps interpreting keys as
        // pairing-mode keys, which leaves the UI and input layer out of sync.
        // Split layout: adaptive width based on number of target preview screens.
        let preview_targets = app.pairing.preview_matches.len();
        let (left_percent, right_percent) = calculate_preview_split(preview_targets);
        let split = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(left_percent),  // Selected wallpaper
                Constraint::Percentage(right_percent), // Pairing preview
            ])
            .split(chunks[chunk_idx]);

        draw_carousel_single(f, app, split[0], &theme);
        draw_pairing_panel(f, app, split[1], &theme);
    } else {
        draw_carousel(f, app, chunks[chunk_idx], &theme);
    }
    chunk_idx += 1;

    if app.ui.show_colors {
        draw_color_palette(f, app, chunks[chunk_idx], &theme);
        chunk_idx += 1;
    }

    draw_footer(f, app, chunks[chunk_idx], &theme);

    // Draw popups on top
    if app.ui.show_color_picker {
        draw_color_picker(f, app, area, &theme);
    } else if app.ui.show_help {
        draw_help_popup(f, area, &theme);
    }

    // Draw undo popup (always on top if active)
    if app.pairing.history.can_undo() {
        draw_undo_popup(f, app, area, &theme);
    }
}

/// Build vertical layout constraints based on UI state.
/// Returns constraints for: header, [error], carousel, [colors], footer.
fn build_layout_constraints(show_colors: bool, has_error: bool) -> Vec<Constraint> {
    match (show_colors, has_error) {
        (true, true) => vec![
            Constraint::Length(2), // Header
            Constraint::Length(1), // Error
            Constraint::Min(7),    // Carousel
            Constraint::Length(3), // Color palette
            Constraint::Length(2), // Footer
        ],
        (true, false) => vec![
            Constraint::Length(2), // Header
            Constraint::Min(8),    // Carousel
            Constraint::Length(3), // Color palette
            Constraint::Length(2), // Footer
        ],
        (false, true) => vec![
            Constraint::Length(2), // Header
            Constraint::Length(1), // Error
            Constraint::Min(9),    // Carousel
            Constraint::Length(2), // Footer
        ],
        (false, false) => vec![
            Constraint::Length(2), // Header
            Constraint::Min(10),   // Carousel
            Constraint::Length(2), // Footer
        ],
    }
}

/// Calculate the left/right percentage split for the pairing preview layout.
/// Returns `(left_percent, right_percent)`.
fn calculate_preview_split(target_screen_count: usize) -> (u16, u16) {
    let right_percent: u16 = match target_screen_count {
        0 | 1 => 45,
        2 => 50,
        _ => 55,
    };
    (100 - right_percent, right_percent)
}

fn center_vertically(area: Rect, height: u16) -> Rect {
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(area.x, y, area.width, height)
}

fn fit_aspect(max_w: u16, max_h: u16, aspect_w: u32, aspect_h: u32) -> (u16, u16) {
    if max_w == 0 || max_h == 0 || aspect_w == 0 || aspect_h == 0 {
        return (0, 0);
    }

    let width_limited_h = ((max_w as u32) * aspect_h / aspect_w) as u16;
    if width_limited_h <= max_h {
        (max_w, width_limited_h.max(1))
    } else {
        let height = max_h;
        let width = ((height as u32) * aspect_w / aspect_h) as u16;
        (width.max(1), height)
    }
}

fn draw_color_palette(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let Some(wp) = app.selected_wallpaper() else {
        let text = Paragraph::new("No color data")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
        return;
    };

    if wp.colors.is_empty() {
        let text = Paragraph::new("No color data")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        f.render_widget(text, area);
        return;
    }

    // Build color swatches
    let mut spans = vec![Span::styled(
        "Colors: ",
        Style::default().fg(theme.fg_secondary),
    )];

    for (i, color_hex) in wp.colors.iter().enumerate() {
        // Parse hex color
        if let Some(color) = parse_hex_color(color_hex) {
            // Color block using background color
            spans.push(Span::styled("  █████  ", Style::default().fg(color)));
            spans.push(Span::styled(color_hex, Style::default().fg(theme.fg_muted)));

            if i < wp.colors.len() - 1 {
                spans.push(Span::styled(" ", Style::default()));
            }
        }
    }

    if !wp.tags.is_empty() {
        spans.push(Span::styled(
            " │ ",
            Style::default().fg(theme.fg_muted),
        ));
        spans.push(Span::styled(
            "Tags: ",
            Style::default().fg(theme.fg_secondary),
        ));
        for (i, tag) in wp.tags.iter().enumerate() {
            spans.push(Span::styled(
                format!("#{}", tag),
                Style::default().fg(theme.accent_highlight),
            ));
            if i < wp.tags.len() - 1 {
                spans.push(Span::styled(" ", Style::default()));
            }
        }
    }

    let line = Line::from(spans);
    let paragraph = Paragraph::new(line).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

fn parse_hex_color(hex: &str) -> Option<ratatui::style::Color> {
    crate::utils::hex_to_rgb(hex).map(|(r, g, b)| ratatui::style::Color::Rgb(r, g, b))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── build_layout_constraints ──────────────────────────────────────────────

    #[test]
    fn test_constraints_no_colors_no_error() {
        let cs = build_layout_constraints(false, false);
        // Expect: header, carousel, footer  (3 items)
        assert_eq!(cs.len(), 3);
        assert_eq!(cs[0], Constraint::Length(2)); // header
        assert_eq!(cs[2], Constraint::Length(2)); // footer
    }

    #[test]
    fn test_constraints_with_colors_no_error() {
        let cs = build_layout_constraints(true, false);
        // Expect: header, carousel, colors, footer (4 items)
        assert_eq!(cs.len(), 4);
        assert_eq!(cs[0], Constraint::Length(2)); // header
        assert_eq!(cs[2], Constraint::Length(3)); // color palette
        assert_eq!(cs[3], Constraint::Length(2)); // footer
    }

    #[test]
    fn test_constraints_no_colors_with_error() {
        let cs = build_layout_constraints(false, true);
        // Expect: header, error, carousel, footer (4 items)
        assert_eq!(cs.len(), 4);
        assert_eq!(cs[0], Constraint::Length(2)); // header
        assert_eq!(cs[1], Constraint::Length(1)); // error
        assert_eq!(cs[3], Constraint::Length(2)); // footer
    }

    #[test]
    fn test_constraints_with_colors_with_error() {
        let cs = build_layout_constraints(true, true);
        // Expect: header, error, carousel, colors, footer (5 items)
        assert_eq!(cs.len(), 5);
        assert_eq!(cs[0], Constraint::Length(2)); // header
        assert_eq!(cs[1], Constraint::Length(1)); // error
        assert_eq!(cs[3], Constraint::Length(3)); // color palette
        assert_eq!(cs[4], Constraint::Length(2)); // footer
    }

    // ── calculate_preview_split ───────────────────────────────────────────────

    #[test]
    fn test_preview_split_zero_targets() {
        let (left, right) = calculate_preview_split(0);
        assert_eq!((left, right), (55, 45));
    }

    #[test]
    fn test_preview_split_one_target() {
        let (left, right) = calculate_preview_split(1);
        assert_eq!((left, right), (55, 45));
    }

    #[test]
    fn test_preview_split_two_targets() {
        let (left, right) = calculate_preview_split(2);
        assert_eq!((left, right), (50, 50));
    }

    #[test]
    fn test_preview_split_many_targets() {
        let (left, right) = calculate_preview_split(5);
        assert_eq!((left, right), (45, 55));
    }

    #[test]
    fn test_preview_split_sums_to_100() {
        for n in [0, 1, 2, 3, 10] {
            let (l, r) = calculate_preview_split(n);
            assert_eq!(l + r, 100, "split must sum to 100 for n={n}");
        }
    }
}
