use super::center_vertically;
use crate::app::App;
use crate::ui::theme::FrostTheme;
use crate::utils::ColorHarmony;
use ratatui::{
    layout::{Alignment, Rect},
    style::{Modifier, Style},
    widgets::{Block, Borders, Paragraph},
    Frame,
};
use ratatui_image::StatefulImage;
use std::collections::HashMap;

pub(super) fn draw_pairing_panel(f: &mut Frame, app: &mut App, area: Rect, theme: &FrostTheme) {
    let alternatives = app.pairing_preview_alternatives();
    let preview_idx = app.pairing.preview_idx;

    // Panel border
    let title = format!(
        " Pair {}/{} · Style {} ",
        preview_idx + 1,
        alternatives,
        app.pairing.style_mode.display_name()
    );
    let block = Block::default()
        .title(title)
        .title_style(
            Style::default()
                .fg(theme.accent_highlight)
                .add_modifier(Modifier::BOLD),
        )
        .borders(Borders::ALL)
        .border_style(Style::default().fg(theme.success))
        .style(Style::default().bg(theme.bg_dark));

    let inner = block.inner(area);
    f.render_widget(block, area);

    if app.pairing.preview_matches.is_empty() {
        let text = Paragraph::new("No suggestions")
            .style(Style::default().fg(theme.fg_muted))
            .alignment(Alignment::Center);
        f.render_widget(text, center_vertically(inner, 1));
        return;
    }

    // Build once to avoid repeated O(n) scans for each preview row.
    let cache_index_by_path: HashMap<&std::path::Path, usize> = app
        .cache
        .wallpapers
        .iter()
        .enumerate()
        .map(|(idx, wp)| (wp.path.as_path(), idx))
        .collect();

    // Collect preview data: (screen_name, cache_idx, filename, harmony)
    let preview_data: Vec<(String, Option<usize>, String, ColorHarmony)> = app
        .pairing
        .preview_matches
        .iter()
        .map(|(screen_name, matches)| {
            let idx = preview_idx.min(matches.len().saturating_sub(1));
            if let Some((path, _, harmony)) = matches.get(idx) {
                let cache_idx = cache_index_by_path.get(path.as_path()).copied();
                let filename = path
                    .file_stem()
                    .and_then(|n| n.to_str())
                    .unwrap_or("?")
                    .to_string();
                (screen_name.clone(), cache_idx, filename, *harmony)
            } else {
                (
                    screen_name.clone(),
                    None,
                    "?".to_string(),
                    ColorHarmony::None,
                )
            }
        })
        .collect();

    // Request all thumbnails
    for (_, cache_idx, _, _) in &preview_data {
        if let Some(ci) = cache_idx {
            app.request_thumbnail(*ci);
        }
    }

    // Calculate layout dynamically for current terminal size.
    let num_items = preview_data.len();
    let available_height = inner.height.max(1);
    let slot_h = if num_items == 1 {
        available_height
    } else {
        (available_height / num_items as u16).max(4)
    };
    let used_height = slot_h.saturating_mul(num_items as u16);
    let mut y_offset = inner.y + available_height.saturating_sub(used_height) / 2;

    for (screen_name, cache_idx, filename, harmony) in preview_data {
        if y_offset + slot_h > inner.y + inner.height || slot_h < 2 {
            break;
        }

        let header_h = if num_items > 1 { 1 } else { 0 };

        // Screen name header with harmony indicator
        let harmony_icon = match harmony {
            ColorHarmony::Analogous => "~",          // Similar
            ColorHarmony::Complementary => "◐",      // Opposite
            ColorHarmony::Triadic => "△",            // Triangle
            ColorHarmony::SplitComplementary => "⋈", // Split
            ColorHarmony::None => "",
        };
        let screen_short: String = screen_name
            .chars()
            .take(inner.width.saturating_sub(4) as usize)
            .collect();
        let header_text = if harmony_icon.is_empty() {
            screen_short
        } else {
            format!("{} {}", harmony_icon, screen_short)
        };
        if header_h > 0 {
            let header = Paragraph::new(header_text)
                .style(
                    Style::default()
                        .fg(theme.accent_secondary)
                        .add_modifier(Modifier::BOLD),
                )
                .alignment(Alignment::Center);
            f.render_widget(header, Rect::new(inner.x, y_offset, inner.width, 1));
        }

        let content_area = Rect::new(
            inner.x,
            y_offset + header_h,
            inner.width,
            slot_h.saturating_sub(header_h),
        );
        if content_area.width < 3 || content_area.height < 3 {
            y_offset += slot_h;
            continue;
        }
        let thumb_area = content_area;

        let thumb_block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border))
            .style(Style::default().bg(theme.bg_medium));
        let thumb_inner = thumb_block.inner(thumb_area);
        f.render_widget(thumb_block, thumb_area);

        // Render thumbnail
        if let Some(ci) = cache_idx {
            if let Some(protocol) = app.get_thumbnail(ci) {
                let image = StatefulImage::new(None);
                f.render_stateful_widget(image, thumb_inner, protocol);
            } else {
                // Fallback: filename
                let name_short: String =
                    filename.chars().take(thumb_inner.width as usize).collect();
                let label = Paragraph::new(name_short)
                    .style(Style::default().fg(theme.fg_secondary))
                    .alignment(Alignment::Center);
                f.render_widget(label, center_vertically(thumb_inner, 1));
            }
        }

        y_offset += slot_h;
    }
}
