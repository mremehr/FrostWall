use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

fn preview_digit_index(code: KeyCode) -> Option<usize> {
    let KeyCode::Char(digit) = code else {
        return None;
    };
    if !digit.is_ascii_digit() {
        return None;
    }

    Some(if digit == '0' {
        9
    } else {
        (digit as u8 - b'1') as usize
    })
}

fn handle_help_popup(app: &mut App, code: KeyCode) -> bool {
    if !app.ui.show_help {
        return false;
    }

    if matches!(code, KeyCode::Char('?') | KeyCode::Esc | KeyCode::Enter) {
        app.ui.show_help = false;
    }
    true
}

fn handle_color_picker_popup(app: &mut App, code: KeyCode) -> bool {
    if !app.ui.show_color_picker {
        return false;
    }

    match code {
        KeyCode::Esc | KeyCode::Char('C') => {
            app.ui.show_color_picker = false;
        }
        KeyCode::Char('l') | KeyCode::Right => {
            app.color_picker_next();
        }
        KeyCode::Char('h') | KeyCode::Left => {
            app.color_picker_prev();
        }
        KeyCode::Enter => {
            app.apply_color_filter();
        }
        KeyCode::Char('x') | KeyCode::Backspace => {
            app.clear_color_filter();
            app.ui.show_color_picker = false;
        }
        _ => {}
    }

    true
}

fn handle_pairing_preview_popup(app: &mut App, code: KeyCode) -> bool {
    if !app.pairing.show_preview {
        return false;
    }

    match code {
        KeyCode::Esc | KeyCode::Char('p') => {
            app.pairing.show_preview = false;
        }
        KeyCode::Char('l') | KeyCode::Char('n') | KeyCode::Right => {
            app.pairing_preview_next();
        }
        KeyCode::Char('h') | KeyCode::Char('N') | KeyCode::Left => {
            app.pairing_preview_prev();
        }
        KeyCode::Enter => {
            if let Err(error) = app.apply_pairing_preview() {
                app.ui.status_message = Some(error.to_string());
            }
        }
        KeyCode::Char('y') => {
            app.toggle_pairing_style_mode();
        }
        _ => {
            if let Some(index) = preview_digit_index(code) {
                let max = app.pairing_preview_alternatives();
                if index < max {
                    app.pairing.preview_idx = index;
                }
            }
        }
    }

    true
}

fn handle_command_mode(app: &mut App, code: KeyCode) -> bool {
    if !app.ui.command_mode {
        return false;
    }

    match code {
        KeyCode::Esc => app.exit_command_mode(),
        KeyCode::Enter => app.execute_command(),
        KeyCode::Backspace => app.command_backspace(),
        KeyCode::Char(character) => app.command_input(character),
        _ => {}
    }

    true
}

fn handle_bound_key_event(app: &mut App, code: KeyCode) -> bool {
    let kb = &app.config.keybindings;

    if kb.matches(code, &kb.quit) || code == KeyCode::Esc {
        app.ui.should_quit = true;
    } else if kb.matches(code, &kb.next) || code == KeyCode::Right {
        app.next_wallpaper();
    } else if kb.matches(code, &kb.prev) || code == KeyCode::Left {
        app.prev_wallpaper();
    } else if kb.matches(code, &kb.next_screen) {
        app.next_screen();
    } else if kb.matches(code, &kb.prev_screen) {
        app.prev_screen();
    } else if kb.matches(code, &kb.apply) {
        if let Err(error) = app.apply_wallpaper() {
            app.ui.status_message = Some(error.to_string());
        }
    } else if kb.matches(code, &kb.random) {
        if let Err(error) = app.random_wallpaper() {
            app.ui.status_message = Some(error.to_string());
        }
    } else if kb.matches(code, &kb.toggle_match) {
        app.toggle_match_mode();
    } else if kb.matches(code, &kb.toggle_resize) {
        app.toggle_resize_mode();
    } else {
        return false;
    }

    true
}

fn handle_fallback_key_event(app: &mut App, code: KeyCode) {
    match code {
        KeyCode::Char(':') => app.enter_command_mode(),
        KeyCode::Char('?') => app.toggle_help(),
        KeyCode::Char('s') => app.toggle_sort_mode(),
        KeyCode::Char('a') | KeyCode::Char('A') => app.toggle_aspect_sort(),
        KeyCode::Char('c') => app.toggle_colors(),
        KeyCode::Char('C') => app.toggle_color_picker(),
        KeyCode::Char('p') => app.toggle_pairing_preview(),
        KeyCode::Char('t') => app.cycle_tag_filter(),
        KeyCode::Char('T') => app.clear_tag_filter(),
        KeyCode::Char('w') => {
            if let Err(error) = app.export_pywal() {
                app.ui.status_message = Some(format!("pywal: {error}"));
            }
        }
        KeyCode::Char('i') => app.toggle_thumbnail_protocol_mode(),
        KeyCode::Char('W') => app.toggle_pywal_export(),
        KeyCode::Char('u') => {
            if let Err(error) = app.do_undo() {
                app.ui.status_message = Some(format!("Undo: {error}"));
            }
        }
        KeyCode::Char('R') => match app.rescan() {
            Ok(message) => app.ui.status_message = Some(format!("Rescan: {message}")),
            Err(error) => app.ui.status_message = Some(format!("Rescan: {error}")),
        },
        _ => {}
    }
}

pub(super) fn handle_key_event(app: &mut App, key: KeyEvent) {
    let code = key.code;
    if handle_help_popup(app, code)
        || handle_color_picker_popup(app, code)
        || handle_pairing_preview_popup(app, code)
        || handle_command_mode(app, code)
    {
        return;
    }

    if !handle_bound_key_event(app, code) {
        handle_fallback_key_event(app, code);
    }
}
