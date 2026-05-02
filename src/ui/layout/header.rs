use super::parse_hex_color;
use crate::app::App;
use crate::ui::theme::FrostTheme;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
    Frame,
};
use ratatui_image::picker::ProtocolType;

pub(super) fn draw_error(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    if let Some(error) = &app.ui.status_message {
        let error_line = Line::from(vec![
            Span::styled("⚠ ", Style::default().fg(theme.warning)),
            Span::styled(error, Style::default().fg(theme.warning)),
        ]);
        let paragraph = Paragraph::new(error_line).alignment(Alignment::Center);
        f.render_widget(paragraph, area);
    }
}

// Width thresholds (in cells) above which lower-priority tags become visible.
// Chosen so the always-shown core (FrostWall + screen + count + match-mode +
// any active filters) never gets clipped on common terminal widths.
const HEADER_SHOW_RESIZE_MIN: u16 = 80;
const HEADER_SHOW_SORT_ASPECT_MIN: u16 = 100;
const HEADER_SHOW_PROTOCOL_MIN: u16 = 120;
const HEADER_SHOW_SCREEN_INFO_MIN: u16 = 70;

pub(super) fn draw_header(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let cols = area.width;

    let screen_info = if cols >= HEADER_SHOW_SCREEN_INFO_MIN {
        Some(if let Some(screen) = app.selected_screen() {
            format!(
                "{} · {}x{} · {:?}",
                screen.name, screen.width, screen.height, screen.aspect_category
            )
        } else {
            "No screens".to_string()
        })
    } else {
        None
    };

    let clamped = app
        .selection
        .wallpaper_idx
        .min(app.selection.filtered_wallpapers.len().saturating_sub(1));
    let total = app.selection.filtered_wallpapers.len();
    let count_info = if total == 0 {
        "0/0".to_string()
    } else {
        format!("{}/{}", clamped + 1, total)
    };

    let match_mode = app.config.display.match_mode.display_name();

    let sep = || Span::styled(" │ ", Style::default().fg(theme.fg_muted));

    let mut header_spans = vec![Span::styled(
        " FrostWall ",
        Style::default()
            .fg(theme.accent_highlight)
            .add_modifier(Modifier::BOLD),
    )];

    if let Some(info) = screen_info {
        header_spans.push(Span::styled("│ ", Style::default().fg(theme.fg_muted)));
        header_spans.push(Span::styled(info, Style::default().fg(theme.fg_secondary)));
        header_spans.push(sep());
    } else {
        header_spans.push(sep());
    }

    header_spans.push(Span::styled(
        count_info,
        Style::default().fg(theme.accent_primary),
    ));
    header_spans.push(sep());
    header_spans.push(Span::styled(
        format!("[{}]", match_mode),
        Style::default().fg(theme.accent_primary),
    ));

    if cols >= HEADER_SHOW_RESIZE_MIN {
        let resize_mode = app.config.display.resize_mode.display_name();
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            format!("[{}]", resize_mode),
            Style::default().fg(theme.accent_secondary),
        ));
    }

    if cols >= HEADER_SHOW_SORT_ASPECT_MIN {
        let sort_mode = app.filters.sort_mode.display_name();
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            format!("[⇅{}]", sort_mode),
            Style::default().fg(theme.fg_secondary),
        ));
        let aspect_sort = if app.filters.aspect_sort_enabled {
            "asp:on"
        } else {
            "asp:off"
        };
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            format!("[{}]", aspect_sort),
            Style::default().fg(if app.filters.aspect_sort_enabled {
                theme.accent_highlight
            } else {
                theme.fg_muted
            }),
        ));
    }

    if cols >= HEADER_SHOW_PROTOCOL_MIN {
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            format!("[img:{}]", thumbnail_protocol_label(app)),
            Style::default().fg(theme.fg_secondary),
        ));
    }

    // Always show contextual indicators (filters/exports/suggestions) — they
    // describe state the user just toggled and would surprise them if hidden.
    if let Some(tag) = &app.filters.active_tag {
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            format!("[#{}]", tag),
            Style::default().fg(theme.accent_highlight),
        ));
    }

    if let Some(color) = &app.filters.active_color {
        header_spans.push(Span::raw(" "));
        if let Some(c) = parse_hex_color(color) {
            header_spans.push(Span::styled("█", Style::default().fg(c)));
        }
        header_spans.push(Span::styled(
            format!("[{}]", color),
            Style::default().fg(theme.fg_secondary),
        ));
    }

    if app.ui.pywal_export {
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled("[wal]", Style::default().fg(theme.success)));
    }

    if !app.pairing.suggestions.is_empty() {
        header_spans.push(Span::raw(" "));
        header_spans.push(Span::styled(
            format!("[⚡{}]", app.pairing.suggestions.len()),
            Style::default().fg(theme.success),
        ));
    }

    let paragraph = Paragraph::new(Line::from(header_spans)).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

pub(super) fn draw_footer(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    // Command mode - show command input line
    if app.ui.command_mode {
        let cmd_line = Line::from(vec![
            Span::styled(
                ":",
                Style::default()
                    .fg(theme.accent_primary)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                &app.ui.command_buffer,
                Style::default().fg(theme.fg_primary),
            ),
            Span::styled("█", Style::default().fg(theme.accent_primary)), // Cursor
        ]);
        let paragraph = Paragraph::new(cmd_line);
        f.render_widget(paragraph, area);
        return;
    }

    // Pairing preview mode - show pairing-specific help
    if app.pairing.show_preview {
        let sep = || Span::styled(" │ ", Style::default().fg(theme.fg_muted));

        let help = Line::from(vec![
            Span::styled("←/→", Style::default().fg(theme.success)),
            Span::styled(" cycle", Style::default().fg(theme.fg_muted)),
            sep(),
            Span::styled("1-9,0", Style::default().fg(theme.success)),
            Span::styled(" select", Style::default().fg(theme.fg_muted)),
            sep(),
            Span::styled("Enter", Style::default().fg(theme.success)),
            Span::styled(" apply", Style::default().fg(theme.fg_muted)),
            sep(),
            Span::styled("y", Style::default().fg(theme.success)),
            Span::styled(" style", Style::default().fg(theme.fg_muted)),
            sep(),
            Span::styled("i", Style::default().fg(theme.success)),
            Span::styled(" img", Style::default().fg(theme.fg_muted)),
            sep(),
            Span::styled("p/Esc", Style::default().fg(theme.success)),
            Span::styled(" close", Style::default().fg(theme.fg_muted)),
        ]);
        let paragraph = Paragraph::new(help).alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    draw_help_line(f, app, area, theme);
}

pub(super) fn draw_help_line(f: &mut Frame, _app: &App, area: Rect, theme: &FrostTheme) {
    let sep = || Span::styled(" │ ", Style::default().fg(theme.fg_muted));

    let help = Line::from(vec![
        Span::styled("←/→", Style::default().fg(theme.accent_primary)),
        Span::styled(" nav", Style::default().fg(theme.fg_muted)),
        sep(),
        Span::styled("Enter", Style::default().fg(theme.accent_primary)),
        Span::styled(" apply", Style::default().fg(theme.fg_muted)),
        sep(),
        Span::styled("p", Style::default().fg(theme.accent_primary)),
        Span::styled(" pair", Style::default().fg(theme.fg_muted)),
        sep(),
        Span::styled(":", Style::default().fg(theme.accent_primary)),
        Span::styled(" cmd", Style::default().fg(theme.fg_muted)),
        sep(),
        Span::styled("?", Style::default().fg(theme.accent_primary)),
        Span::styled(" help", Style::default().fg(theme.fg_muted)),
        sep(),
        Span::styled("a", Style::default().fg(theme.accent_primary)),
        Span::styled(" aspect", Style::default().fg(theme.fg_muted)),
        sep(),
        Span::styled("i", Style::default().fg(theme.accent_primary)),
        Span::styled(" img", Style::default().fg(theme.fg_muted)),
        sep(),
        Span::styled("q", Style::default().fg(theme.accent_primary)),
        Span::styled(" quit", Style::default().fg(theme.fg_muted)),
    ]);

    let paragraph = Paragraph::new(help).alignment(Alignment::Center);
    f.render_widget(paragraph, area);
}

pub(super) fn thumbnail_protocol_label(app: &App) -> &'static str {
    app.thumbnails
        .image_picker
        .as_ref()
        .map(|p| match p.protocol_type {
            ProtocolType::Halfblocks => "HB",
            ProtocolType::Sixel => "SIX",
            ProtocolType::Kitty => "KTY",
            ProtocolType::Iterm2 => "IT2",
        })
        .unwrap_or("N/A")
}
