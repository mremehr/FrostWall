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

pub(super) fn draw_header(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let screen_info = if let Some(screen) = app.selected_screen() {
        format!(
            "{} · {}x{} · {:?}",
            screen.name, screen.width, screen.height, screen.aspect_category
        )
    } else {
        "No screens".to_string()
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

    // Show current modes
    let match_mode = app.config.display.match_mode.display_name();
    let resize_mode = app.config.display.resize_mode.display_name();
    let sort_mode = app.filters.sort_mode.display_name();
    let thumb_protocol = thumbnail_protocol_label(app);

    let mut header_spans = vec![Span::styled(
        " FrostWall ",
        Style::default()
            .fg(theme.accent_highlight)
            .add_modifier(Modifier::BOLD),
    )];

    header_spans.extend(vec![
        Span::styled("│ ", Style::default().fg(theme.fg_muted)),
        Span::styled(screen_info, Style::default().fg(theme.fg_secondary)),
        Span::styled(" │ ", Style::default().fg(theme.fg_muted)),
        Span::styled(count_info, Style::default().fg(theme.accent_primary)),
        Span::styled(" │ ", Style::default().fg(theme.fg_muted)),
        Span::styled(
            format!("[{}]", match_mode),
            Style::default().fg(theme.accent_primary),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            format!("[{}]", resize_mode),
            Style::default().fg(theme.accent_secondary),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            format!("[⇅{}]", sort_mode),
            Style::default().fg(theme.fg_secondary),
        ),
        Span::styled(" ", Style::default()),
        Span::styled(
            format!("[img:{}]", thumb_protocol),
            Style::default().fg(theme.fg_secondary),
        ),
    ]);

    // Tag filter indicator
    if let Some(tag) = &app.filters.active_tag {
        header_spans.push(Span::styled(" ", Style::default()));
        header_spans.push(Span::styled(
            format!("[#{}]", tag),
            Style::default().fg(theme.accent_highlight),
        ));
    }

    // Color filter indicator
    if let Some(color) = &app.filters.active_color {
        header_spans.push(Span::styled(" ", Style::default()));
        if let Some(c) = parse_hex_color(color) {
            header_spans.push(Span::styled("█", Style::default().fg(c)));
        }
        header_spans.push(Span::styled(
            format!("[{}]", color),
            Style::default().fg(theme.fg_secondary),
        ));
    }

    // Pywal indicator
    if app.ui.pywal_export {
        header_spans.push(Span::styled(" ", Style::default()));
        header_spans.push(Span::styled("[wal]", Style::default().fg(theme.success)));
    }

    // Pairing suggestions indicator
    if !app.pairing.suggestions.is_empty() {
        header_spans.push(Span::styled(" ", Style::default()));
        header_spans.push(Span::styled(
            format!("[⚡{}]", app.pairing.suggestions.len()),
            Style::default().fg(theme.success),
        ));
    }

    let header = Line::from(header_spans);

    let paragraph = Paragraph::new(header).alignment(Alignment::Center);
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
        let sep = Span::styled(" │ ", Style::default().fg(theme.fg_muted));
        let thumb_protocol = thumbnail_protocol_label(app);

        let help = Line::from(vec![
            Span::styled(
                format!("img:{}", thumb_protocol),
                Style::default().fg(theme.fg_secondary),
            ),
            sep.clone(),
            Span::styled("←/→", Style::default().fg(theme.success)),
            Span::styled(" cycle", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("1-9,0", Style::default().fg(theme.success)),
            Span::styled(" select", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("Enter", Style::default().fg(theme.success)),
            Span::styled(" apply", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("y", Style::default().fg(theme.success)),
            Span::styled(" style", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("i", Style::default().fg(theme.success)),
            Span::styled(" img", Style::default().fg(theme.fg_muted)),
            sep.clone(),
            Span::styled("p/Esc", Style::default().fg(theme.success)),
            Span::styled(" close", Style::default().fg(theme.fg_muted)),
        ]);
        let paragraph = Paragraph::new(help).alignment(Alignment::Center);
        f.render_widget(paragraph, area);
        return;
    }

    draw_help_line(f, app, area, theme);
}

pub(super) fn draw_help_line(f: &mut Frame, app: &App, area: Rect, theme: &FrostTheme) {
    let sep = Span::styled(" │ ", Style::default().fg(theme.fg_muted));
    let thumb_protocol = thumbnail_protocol_label(app);

    let help = Line::from(vec![
        Span::styled(
            format!("img:{}", thumb_protocol),
            Style::default().fg(theme.fg_secondary),
        ),
        sep.clone(),
        Span::styled("←/→", Style::default().fg(theme.accent_primary)),
        Span::styled(" nav", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("Enter", Style::default().fg(theme.accent_primary)),
        Span::styled(" apply", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("p", Style::default().fg(theme.accent_primary)),
        Span::styled(" pair", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled(":", Style::default().fg(theme.accent_primary)),
        Span::styled(" cmd", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("?", Style::default().fg(theme.accent_primary)),
        Span::styled(" help", Style::default().fg(theme.fg_muted)),
        sep.clone(),
        Span::styled("i", Style::default().fg(theme.accent_primary)),
        Span::styled(" img", Style::default().fg(theme.fg_muted)),
        sep.clone(),
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
