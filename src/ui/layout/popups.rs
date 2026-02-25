use super::{center_vertically, parse_hex_color};
use crate::app::App;
use crate::ui::theme::FrostTheme;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph},
    Frame,
};

pub(super) fn draw_carousel_placeholder(f: &mut Frame, area: Rect, theme: &FrostTheme) {
    // Simple placeholder when popup is active (images would render over popup)
    let text = Paragraph::new("(popup active)")
        .style(Style::default().fg(theme.fg_muted))
        .alignment(Alignment::Center);
    let centered = center_vertically(area, 1);
    f.render_widget(text, centered);
}

pub(super) fn draw_color_picker(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let colors = &app.filters.available_colors;
    if colors.is_empty() {
        return;
    }

    // Calculate popup size based on color count
    let cols = 8; // Colors per row
    let rows = colors.len().div_ceil(cols);
    let popup_width = 60.min(area.width.saturating_sub(4));
    let popup_height = (rows as u16 * 2 + 6).min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear background
    let clear = Block::default().style(Style::default().bg(theme.bg_dark));
    f.render_widget(clear, popup_area);

    // Popup border
    let title = if let Some(ref color) = app.filters.active_color {
        format!(" Color Filter [{}] ", color)
    } else {
        " Color Filter ".to_string()
    };

    let block = Block::default()
        .title(title)
        .title_style(
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_primary))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Draw color swatches in a grid
    let swatch_width = 6;
    let swatch_height = 1;
    let spacing = 1;

    for (i, color_hex) in colors.iter().enumerate() {
        let col = i % cols;
        let row = i / cols;

        let x = inner.x + (col as u16) * (swatch_width + spacing);
        let y = inner.y + (row as u16) * (swatch_height + spacing);

        if x + swatch_width > inner.x + inner.width || y + swatch_height > inner.y + inner.height {
            continue;
        }

        let swatch_area = Rect::new(x, y, swatch_width, swatch_height);

        // Parse color
        let color = parse_hex_color(color_hex).unwrap_or(theme.fg_muted);

        // Highlight selected
        let is_selected = i == app.filters.color_picker_idx;
        let style = if is_selected {
            Style::default()
                .bg(color)
                .fg(theme.bg_dark)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(color)
        };

        let text = if is_selected {
            "▶▶▶▶"
        } else {
            "████"
        };
        let swatch = Paragraph::new(text).style(style);
        f.render_widget(swatch, swatch_area);
    }

    // Footer with instructions
    let footer_y = inner.y + inner.height.saturating_sub(2);
    if footer_y > inner.y {
        let footer_area = Rect::new(inner.x, footer_y, inner.width, 2);
        let footer = Line::from(vec![
            Span::styled("←/→", Style::default().fg(theme.accent_primary)),
            Span::styled(" select ", Style::default().fg(theme.fg_muted)),
            Span::styled("Enter", Style::default().fg(theme.accent_primary)),
            Span::styled(" apply ", Style::default().fg(theme.fg_muted)),
            Span::styled("x", Style::default().fg(theme.accent_primary)),
            Span::styled(" clear ", Style::default().fg(theme.fg_muted)),
            Span::styled("Esc", Style::default().fg(theme.accent_primary)),
            Span::styled(" close", Style::default().fg(theme.fg_muted)),
        ]);
        let para = Paragraph::new(footer).alignment(Alignment::Center);
        f.render_widget(para, footer_area);
    }
}

pub(super) fn draw_help_popup(f: &mut Frame, area: Rect, theme: &FrostTheme) {
    // Center the popup
    let popup_width = 50.min(area.width.saturating_sub(4));
    let popup_height = 35.min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear background
    let clear = Block::default().style(Style::default().bg(theme.bg_dark));
    f.render_widget(clear, popup_area);

    // Popup border
    let block = Block::default()
        .title(" ❄️ FrostWall Help ")
        .title_style(
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.accent_primary))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Help content
    let help_text = vec![
        Line::from(vec![Span::styled(
            "Navigation",
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  h/←     ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                "Previous wallpaper",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  l/→     ", Style::default().fg(theme.accent_primary)),
            Span::styled("Next wallpaper", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  Tab     ", Style::default().fg(theme.accent_primary)),
            Span::styled("Next screen", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  S-Tab   ", Style::default().fg(theme.accent_primary)),
            Span::styled("Previous screen", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Actions",
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  Enter   ", Style::default().fg(theme.accent_primary)),
            Span::styled("Apply wallpaper", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  r       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Random wallpaper", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  :       ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                "Command mode (vim-style)",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Commands (:)",
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  :t <tag>", Style::default().fg(theme.accent_primary)),
            Span::styled(" Filter by tag", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  :clear  ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                " Clear all filters",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  :sim    ", Style::default().fg(theme.accent_primary)),
            Span::styled(" Find similar", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  :sort n ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                " Sort (name/date/size)",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  :rescan ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                " Rescan wallpaper dir",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  :pair-reset", Style::default().fg(theme.accent_primary)),
            Span::styled(
                " Rebuild pairing data",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "Options",
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::styled("  m       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Toggle match mode", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  f       ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                "Toggle resize mode",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  s       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Toggle sort mode", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  c       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Show/hide colors", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  t       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Cycle tag filter", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  T       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Clear tag filter", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  C       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Open color picker", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  p       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Pairing preview", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  w       ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                "Export pywal colors",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  W       ", Style::default().fg(theme.accent_primary)),
            Span::styled("Toggle auto pywal", Style::default().fg(theme.fg_secondary)),
        ]),
        Line::from(vec![
            Span::styled("  R       ", Style::default().fg(theme.accent_primary)),
            Span::styled(
                "Rescan wallpaper dir",
                Style::default().fg(theme.fg_secondary),
            ),
        ]),
        Line::from(vec![
            Span::styled("  q/Esc   ", Style::default().fg(theme.accent_primary)),
            Span::styled("Quit", Style::default().fg(theme.fg_secondary)),
        ]),
    ];

    let paragraph = Paragraph::new(help_text);
    f.render_widget(paragraph, inner);
}

/// Draw undo popup at bottom of screen
pub(super) fn draw_undo_popup(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let remaining_secs = app.pairing.history.undo_remaining_secs().unwrap_or(0);
    let message = app
        .pairing
        .history
        .undo_message()
        .unwrap_or("Undo available");

    // Position at bottom center
    let popup_width = 45.min(area.width.saturating_sub(4));
    let popup_height = 3;
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = area.height.saturating_sub(popup_height + 2);

    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    // Clear background
    let clear = Block::default().style(Style::default().bg(theme.bg_dark));
    f.render_widget(clear, popup_area);

    // Popup border
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.warning))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    // Content
    let text = Line::from(vec![
        Span::styled(message, Style::default().fg(theme.fg_primary)),
        Span::styled(" | ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!("Undo (u) {}s", remaining_secs),
            Style::default()
                .fg(theme.warning)
                .add_modifier(Modifier::BOLD),
        ),
    ]);

    let paragraph = Paragraph::new(text).alignment(Alignment::Center);
    f.render_widget(paragraph, inner);
}
