use super::sections::KeybindingsConfig;
use crossterm::event::KeyCode;

impl KeybindingsConfig {
    /// Parse a keybinding string into a KeyCode.
    pub fn parse_key(input: &str) -> Option<KeyCode> {
        let input = input.trim();

        if input.len() == 1 {
            return input.chars().next().map(KeyCode::Char);
        }

        match input.to_lowercase().as_str() {
            "enter" | "return" => Some(KeyCode::Enter),
            "esc" | "escape" => Some(KeyCode::Esc),
            "tab" => Some(KeyCode::Tab),
            "backtab" | "shift+tab" | "s-tab" => Some(KeyCode::BackTab),
            "space" => Some(KeyCode::Char(' ')),
            "backspace" => Some(KeyCode::Backspace),
            "delete" | "del" => Some(KeyCode::Delete),
            "insert" | "ins" => Some(KeyCode::Insert),
            "home" => Some(KeyCode::Home),
            "end" => Some(KeyCode::End),
            "pageup" | "pgup" => Some(KeyCode::PageUp),
            "pagedown" | "pgdn" => Some(KeyCode::PageDown),
            "up" | "arrow_up" => Some(KeyCode::Up),
            "down" | "arrow_down" => Some(KeyCode::Down),
            "left" | "arrow_left" => Some(KeyCode::Left),
            "right" | "arrow_right" => Some(KeyCode::Right),
            "f1" => Some(KeyCode::F(1)),
            "f2" => Some(KeyCode::F(2)),
            "f3" => Some(KeyCode::F(3)),
            "f4" => Some(KeyCode::F(4)),
            "f5" => Some(KeyCode::F(5)),
            "f6" => Some(KeyCode::F(6)),
            "f7" => Some(KeyCode::F(7)),
            "f8" => Some(KeyCode::F(8)),
            "f9" => Some(KeyCode::F(9)),
            "f10" => Some(KeyCode::F(10)),
            "f11" => Some(KeyCode::F(11)),
            "f12" => Some(KeyCode::F(12)),
            _ => None,
        }
    }

    /// Check if a KeyCode matches a keybinding.
    pub fn matches(&self, key: KeyCode, binding: &str) -> bool {
        Self::parse_key(binding) == Some(key)
    }
}
