use super::{center_vertically, parse_hex_color};
use crate::app::App;
use crate::ui::theme::FrostTheme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
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

// Color-picker swatch geometry: each cell is `SWATCH_WIDTH` cells wide
// followed by a one-cell gutter. Heights are 1 cell with a one-row gutter.
const SWATCH_WIDTH: u16 = 6;
const SWATCH_HEIGHT: u16 = 1;
const SWATCH_GAP: u16 = 1;
// Stay short of 16 columns; otherwise the popup hits the screen edge on
// ultrawide monitors and dwarfs the rest of the UI.
const MAX_COLOR_PICKER_COLS: usize = 16;
const MIN_COLOR_PICKER_COLS: usize = 1;
const COLOR_PICKER_PADDING: u16 = 4;
const COLOR_PICKER_CHROME_HEIGHT: u16 = 6;

pub(super) fn draw_color_picker(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let colors = &app.filters.available_colors;
    if colors.is_empty() {
        return;
    }

    // Fit the grid to the available space: prefer wider rows on big terminals,
    // collapse gracefully on small ones. The +SWATCH_GAP in the divisor makes
    // the math account for the trailing gutter on each row.
    let max_inner_width = area.width.saturating_sub(COLOR_PICKER_PADDING + 2);
    let cells_per_swatch = SWATCH_WIDTH + SWATCH_GAP;
    let fit_cols = ((max_inner_width + SWATCH_GAP) / cells_per_swatch) as usize;
    let cols = fit_cols
        .clamp(MIN_COLOR_PICKER_COLS, MAX_COLOR_PICKER_COLS)
        .min(colors.len().max(1));
    let rows = colors.len().div_ceil(cols);

    let grid_width = (cols as u16) * cells_per_swatch - SWATCH_GAP;
    let popup_width = (grid_width + COLOR_PICKER_PADDING).min(area.width.saturating_sub(4));
    let popup_height = ((rows as u16) * (SWATCH_HEIGHT + SWATCH_GAP) + COLOR_PICKER_CHROME_HEIGHT)
        .min(area.height.saturating_sub(4));
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
    let swatch_width = SWATCH_WIDTH;
    let swatch_height = SWATCH_HEIGHT;
    let spacing = SWATCH_GAP;

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

type HelpEntry = (&'static str, &'static str);
type HelpSection = (&'static str, &'static [HelpEntry]);

const NAV_HELP: HelpSection = (
    "Navigation",
    &[
        ("h/←", "Previous wallpaper"),
        ("l/→", "Next wallpaper"),
        ("Tab", "Next screen"),
        ("S-Tab", "Previous screen"),
    ],
);

const ACTIONS_HELP: HelpSection = (
    "Actions",
    &[
        ("Enter", "Apply wallpaper"),
        ("r", "Random wallpaper"),
        (":", "Command mode (vim-style)"),
    ],
);

const COMMANDS_HELP: HelpSection = (
    "Commands (:)",
    &[
        (":t <tag>", "Filter by tag"),
        (":clear", "Clear all filters"),
        (":sim", "Find similar"),
        (":sort n", "Sort (name/date/size)"),
        (":aspect", "Aspect grouping (on/off)"),
        (":rescan", "Rescan wallpaper dir"),
        (":pair-reset", "Rebuild pairing data"),
    ],
);

const OPTIONS_HELP: HelpSection = (
    "Options",
    &[
        ("m", "Toggle match mode"),
        ("f", "Toggle resize mode"),
        ("s", "Toggle sort mode"),
        ("a", "Toggle aspect grouping"),
        ("c", "Show/hide colors"),
        ("t", "Cycle tag filter"),
        ("T", "Clear tag filter"),
        ("C", "Open color picker"),
        ("p", "Pairing preview"),
        ("w", "Export pywal colors"),
        ("W", "Toggle auto pywal"),
        ("R", "Rescan wallpaper dir"),
        ("q/Esc", "Quit"),
    ],
);

const HELP_KEY_COL_WIDTH: usize = 11;

fn append_section(
    out: &mut Vec<Line<'static>>,
    theme: &FrostTheme,
    section: HelpSection,
    leading_blank: bool,
) {
    if leading_blank {
        out.push(Line::from(""));
    }
    out.push(Line::from(Span::styled(
        section.0.to_string(),
        Style::default()
            .fg(theme.accent_highlight)
            .add_modifier(Modifier::BOLD),
    )));
    for (key, desc) in section.1 {
        out.push(Line::from(vec![
            Span::styled(
                format!("  {:<width$}", key, width = HELP_KEY_COL_WIDTH),
                Style::default().fg(theme.accent_primary),
            ),
            Span::styled(
                (*desc).to_string(),
                Style::default().fg(theme.fg_secondary),
            ),
        ]));
    }
}

fn build_help_lines(theme: &FrostTheme, sections: &[HelpSection]) -> Vec<Line<'static>> {
    let mut lines: Vec<Line<'static>> = Vec::new();
    for (i, section) in sections.iter().enumerate() {
        append_section(&mut lines, theme, *section, i > 0);
    }
    lines
}

// Below this width the popup folds back to a single column. Two columns at
// HELP_KEY_COL_WIDTH plus longest descriptions need ~76 cells; we leave a
// little air around 70 to start scrolling instead of clipping descriptions.
const HELP_TWO_COL_MIN_WIDTH: u16 = 76;

pub(super) fn draw_help_popup(f: &mut Frame, area: Rect, theme: &FrostTheme) {
    let two_col = area.width >= HELP_TWO_COL_MIN_WIDTH + 4;

    let (left_lines, right_lines, requested_height, requested_width) = if two_col {
        let left = build_help_lines(theme, &[NAV_HELP, ACTIONS_HELP, COMMANDS_HELP]);
        let right = build_help_lines(theme, &[OPTIONS_HELP]);
        let height = left.len().max(right.len()) as u16;
        (left, right, height, HELP_TWO_COL_MIN_WIDTH)
    } else {
        let combined = build_help_lines(
            theme,
            &[NAV_HELP, ACTIONS_HELP, COMMANDS_HELP, OPTIONS_HELP],
        );
        let height = combined.len() as u16;
        (combined, Vec::new(), height, 50)
    };

    let popup_width = requested_width.min(area.width.saturating_sub(4));
    // +2 for the popup border itself.
    let popup_height = (requested_height + 2).min(area.height.saturating_sub(4));
    let popup_x = (area.width.saturating_sub(popup_width)) / 2;
    let popup_y = (area.height.saturating_sub(popup_height)) / 2;
    let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

    let clear = Block::default().style(Style::default().bg(theme.bg_dark));
    f.render_widget(clear, popup_area);

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

    if two_col {
        let columns = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(55), Constraint::Percentage(45)])
            .split(inner);
        f.render_widget(Paragraph::new(left_lines), columns[0]);
        f.render_widget(Paragraph::new(right_lines), columns[1]);
    } else {
        f.render_widget(Paragraph::new(left_lines), inner);
    }
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
