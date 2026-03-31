use crate::screen::{AspectCategory, Screen};
use crate::wallpaper::{MatchMode, Wallpaper};

impl Wallpaper {
    pub(crate) fn categorize_aspect(width: u32, height: u32) -> AspectCategory {
        if width == 0 || height == 0 {
            return AspectCategory::Square;
        }
        let ratio = width as f32 / height as f32;
        let normalized_ratio = if ratio >= 1.0 { ratio } else { 1.0 / ratio };

        if normalized_ratio >= 2.0 {
            AspectCategory::Ultrawide
        } else if normalized_ratio >= 1.2 {
            if ratio >= 1.0 {
                AspectCategory::Landscape
            } else {
                AspectCategory::Portrait
            }
        } else {
            AspectCategory::Square
        }
    }

    /// Strict match - exact aspect category.
    pub fn matches_screen(&self, screen: &Screen) -> bool {
        self.aspect_category == screen.aspect_category
    }

    /// Flexible match - allows compatible aspect ratios.
    pub fn matches_screen_flexible(&self, screen: &Screen) -> bool {
        use AspectCategory::*;

        match (self.aspect_category, screen.aspect_category) {
            (left, right) if left == right => true,
            (Landscape, Ultrawide) => true,
            (Ultrawide, Landscape) => true,
            (Square, Landscape | Ultrawide) => true,
            (Landscape | Ultrawide, Square) => true,
            (Portrait, Square) | (Square, Portrait) => true,
            _ => false,
        }
    }

    /// Match based on mode.
    pub fn matches_screen_with_mode(&self, screen: &Screen, mode: MatchMode) -> bool {
        match mode {
            MatchMode::Strict => self.matches_screen(screen),
            MatchMode::Flexible => self.matches_screen_flexible(screen),
            MatchMode::All => true,
        }
    }
}
