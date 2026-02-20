use ratatui::style::Color;
use std::fs;

/// Frost theme colors - inspired by Nord/Catppuccin ice palette
#[derive(Clone)]
pub struct FrostTheme {
    // Backgrounds
    pub bg_dark: Color,
    pub bg_medium: Color,

    // Foregrounds
    pub fg_primary: Color,
    pub fg_secondary: Color,
    pub fg_muted: Color,

    // Accents
    pub accent_primary: Color,   // Ice blue
    pub accent_secondary: Color, // Frost teal
    pub accent_highlight: Color, // Golden frost

    // Status
    pub success: Color,
    pub warning: Color,

    // Borders
    pub border: Color,
    pub border_focused: Color,
}

impl FrostTheme {
    /// Frostglow Light - for light terminal backgrounds
    pub fn frostglow_light() -> Self {
        Self {
            // Transparent backgrounds (inherit from terminal)
            bg_dark: Color::Reset,
            bg_medium: Color::Reset,

            // Dark text for light background
            fg_primary: Color::Rgb(10, 15, 20), // #0a0f14 - near black
            fg_secondary: Color::Rgb(26, 45, 66), // #1a2d42 - dark blue-gray
            fg_muted: Color::Rgb(42, 63, 85),   // #2a3f55 - muted dark

            // Ice accents (darker for light bg)
            accent_primary: Color::Rgb(30, 69, 112), // #1e4570 - deep ice blue
            accent_secondary: Color::Rgb(46, 90, 144), // #2e5a90 - bright ice blue
            accent_highlight: Color::Rgb(153, 101, 21), // #996515 - dark golden

            // Status
            success: Color::Rgb(13, 94, 58), // #0d5e3a - dark ice green
            warning: Color::Rgb(153, 101, 21), // #996515 - dark golden

            // Borders
            border: Color::Rgb(184, 212, 241), // #b8d4f1 - soft ice
            border_focused: Color::Rgb(46, 90, 144), // #2e5a90 - bright ice blue
        }
    }

    /// Deep Cracked Ice - for dark terminal backgrounds
    pub fn deep_cracked_ice() -> Self {
        Self {
            // Transparent backgrounds (inherit from terminal)
            bg_dark: Color::Reset,
            bg_medium: Color::Reset,

            // Light text for dark background
            fg_primary: Color::Rgb(245, 250, 255), // #f5faff - bright white
            fg_secondary: Color::Rgb(200, 220, 240), // #c8dcf0 - soft ice
            fg_muted: Color::Rgb(80, 100, 120),    // #506478 - muted gray

            // Ice accents (brighter for dark bg)
            accent_primary: Color::Rgb(100, 200, 255), // #64c8ff - bright ice blue
            accent_secondary: Color::Rgb(80, 250, 150), // #50fa96 - cold green
            accent_highlight: Color::Rgb(255, 215, 95), // #ffd75f - warm yellow

            // Status
            success: Color::Rgb(80, 250, 150), // #50fa96 - cold green
            warning: Color::Rgb(255, 215, 95), // #ffd75f - warm yellow

            // Borders
            border: Color::Rgb(60, 90, 120), // #3c5a78 - dark ice
            border_focused: Color::Rgb(100, 200, 255), // #64c8ff - bright ice blue
        }
    }
}

impl Default for FrostTheme {
    fn default() -> Self {
        if detect_light_theme() {
            Self::frostglow_light()
        } else {
            Self::deep_cracked_ice()
        }
    }
}

/// Auto-detect terminal theme from Alacritty/Kitty config
fn detect_light_theme() -> bool {
    // Check Alacritty theme marker file
    if let Ok(theme_marker) = fs::read_to_string(
        std::env::var("HOME")
            .map(|h| format!("{}/.config/alacritty/.current-theme", h))
            .unwrap_or_default(),
    ) {
        let theme = theme_marker.trim().to_lowercase();
        if theme.contains("light") || theme.contains("frostglow") {
            return true;
        }
        if theme.contains("dark") || theme.contains("cracked") || theme.contains("ice") {
            return false;
        }
    }

    // Parse Kitty config for theme hints
    if let Ok(home) = std::env::var("HOME") {
        let kitty_config = format!("{}/.config/kitty/kitty.conf", home);
        if let Ok(content) = fs::read_to_string(&kitty_config) {
            let header: String = content
                .lines()
                .take(10)
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            if header.contains("frostglow") || header.contains("light") {
                return true;
            }
            if header.contains("deep cracked ice") || header.contains("dark") {
                return false;
            }
        }
    }

    // Parse Alacritty config for theme hints
    if let Ok(home) = std::env::var("HOME") {
        let alacritty_config = format!("{}/.config/alacritty/alacritty.toml", home);
        if let Ok(content) = fs::read_to_string(&alacritty_config) {
            let header: String = content
                .lines()
                .take(10)
                .collect::<Vec<_>>()
                .join(" ")
                .to_lowercase();
            if header.contains("frostglow") || header.contains("light") {
                return true;
            }
            if header.contains("deep cracked ice") || header.contains("dark") {
                return false;
            }
        }
    }

    // Check environment variable
    if let Ok(theme_env) = std::env::var("ALACRITTY_THEME") {
        return theme_env.to_lowercase().contains("light");
    }

    // Default to dark
    false
}

/// Get the current theme based on terminal detection
pub fn frost_theme() -> FrostTheme {
    FrostTheme::default()
}

/// Check if current theme is light (for change detection)
pub fn is_light_theme() -> bool {
    detect_light_theme()
}
