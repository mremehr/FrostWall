//! Time-based wallpaper profiles
//!
//! Automatically select wallpapers based on time of day, preferring
//! appropriate brightness levels and tags for each period.

use chrono::{Local, Timelike};
use serde::{Deserialize, Serialize};

/// Time period of day
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TimePeriod {
    Morning,   // 6:00 - 12:00
    Afternoon, // 12:00 - 18:00
    Evening,   // 18:00 - 22:00
    Night,     // 22:00 - 6:00
}

impl TimePeriod {
    /// Get the current time period
    pub fn current() -> Self {
        let hour = Local::now().hour();
        Self::from_hour(hour)
    }

    /// Get time period from hour (0-23)
    pub fn from_hour(hour: u32) -> Self {
        match hour {
            6..=11 => TimePeriod::Morning,
            12..=17 => TimePeriod::Afternoon,
            18..=21 => TimePeriod::Evening,
            _ => TimePeriod::Night, // 22-23, 0-5
        }
    }

    /// Get the display name
    pub fn name(&self) -> &'static str {
        match self {
            TimePeriod::Morning => "morning",
            TimePeriod::Afternoon => "afternoon",
            TimePeriod::Evening => "evening",
            TimePeriod::Night => "night",
        }
    }

    /// Get the emoji for display
    pub fn emoji(&self) -> &'static str {
        match self {
            TimePeriod::Morning => "🌅",
            TimePeriod::Afternoon => "☀️",
            TimePeriod::Evening => "🌆",
            TimePeriod::Night => "🌙",
        }
    }
}

/// Profile settings for a time period
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeProfileSettings {
    /// Preferred brightness range (0.0-1.0)
    pub brightness_range: (f32, f32),
    /// Preferred tags (in order of priority)
    pub preferred_tags: Vec<String>,
    /// Weight for brightness matching (0.0-1.0)
    pub brightness_weight: f32,
    /// Weight for tag matching (0.0-1.0)
    pub tag_weight: f32,
}

impl Default for TimeProfileSettings {
    fn default() -> Self {
        Self {
            brightness_range: (0.3, 0.7),
            preferred_tags: Vec::new(),
            brightness_weight: 0.5,
            tag_weight: 0.5,
        }
    }
}

/// Time profile configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimeProfiles {
    pub enabled: bool,
    pub morning: TimeProfileSettings,
    pub afternoon: TimeProfileSettings,
    pub evening: TimeProfileSettings,
    pub night: TimeProfileSettings,
}

impl Default for TimeProfiles {
    fn default() -> Self {
        Self {
            enabled: false,
            morning: TimeProfileSettings {
                brightness_range: (0.5, 0.9),
                preferred_tags: vec!["nature".into(), "bright".into(), "pastel".into()],
                brightness_weight: 0.6,
                tag_weight: 0.4,
            },
            afternoon: TimeProfileSettings {
                brightness_range: (0.4, 0.8),
                preferred_tags: vec!["nature".into(), "ocean".into(), "mountain".into()],
                brightness_weight: 0.5,
                tag_weight: 0.5,
            },
            evening: TimeProfileSettings {
                brightness_range: (0.2, 0.6),
                preferred_tags: vec!["sunset".into(), "autumn".into(), "cyberpunk".into()],
                brightness_weight: 0.5,
                tag_weight: 0.5,
            },
            night: TimeProfileSettings {
                brightness_range: (0.0, 0.4),
                preferred_tags: vec!["dark".into(), "space".into(), "minimal".into()],
                brightness_weight: 0.7,
                tag_weight: 0.3,
            },
        }
    }
}

impl TimeProfiles {
    /// Get settings for the current time period
    pub fn current_settings(&self) -> &TimeProfileSettings {
        self.settings_for(TimePeriod::current())
    }

    /// Get settings for a specific time period
    pub fn settings_for(&self, period: TimePeriod) -> &TimeProfileSettings {
        match period {
            TimePeriod::Morning => &self.morning,
            TimePeriod::Afternoon => &self.afternoon,
            TimePeriod::Evening => &self.evening,
            TimePeriod::Night => &self.night,
        }
    }

    /// Score a wallpaper based on current time profile
    /// Returns a score from 0.0 to 1.0
    pub fn score_wallpaper(&self, colors: &[String], tags: &[String]) -> f32 {
        if !self.enabled {
            return 1.0; // No time-based filtering
        }

        let settings = self.current_settings();
        self.score_wallpaper_with_settings(colors, tags, settings)
    }

    /// Score a wallpaper with specific settings
    fn score_wallpaper_with_settings(
        &self,
        colors: &[String],
        tags: &[String],
        settings: &TimeProfileSettings,
    ) -> f32 {
        let mut score = 0.0;

        // Calculate average brightness
        if !colors.is_empty() {
            let avg_brightness: f32 = colors
                .iter()
                .map(|c| crate::utils::color_brightness(c))
                .sum::<f32>()
                / colors.len() as f32;

            // Score based on brightness range
            let (min_b, max_b) = settings.brightness_range;
            let brightness_score = if avg_brightness >= min_b && avg_brightness <= max_b {
                1.0
            } else if avg_brightness < min_b {
                1.0 - (min_b - avg_brightness).min(0.5) * 2.0
            } else {
                1.0 - (avg_brightness - max_b).min(0.5) * 2.0
            };
            score += brightness_score * settings.brightness_weight;
        } else {
            score += 0.5 * settings.brightness_weight;
        }

        // Score based on tag matches
        if !settings.preferred_tags.is_empty() && !tags.is_empty() {
            let mut tag_score = 0.0f32;
            for (priority, preferred) in settings.preferred_tags.iter().enumerate() {
                if tags.iter().any(|t| t.eq_ignore_ascii_case(preferred)) {
                    // Higher priority tags get more weight
                    let weight = 1.0 / (priority + 1) as f32;
                    tag_score = tag_score.max(weight);
                }
            }
            score += tag_score * settings.tag_weight;
        } else {
            score += 0.5 * settings.tag_weight;
        }

        score.clamp(0.0, 1.0)
    }
}

/// Get wallpapers sorted by time profile score
pub fn sort_by_time_profile<'a>(
    wallpapers: &'a [crate::wallpaper::Wallpaper],
    profiles: &TimeProfiles,
) -> Vec<&'a crate::wallpaper::Wallpaper> {
    let mut scored: Vec<_> = wallpapers
        .iter()
        .map(|wp| {
            let score = profiles.score_wallpaper(&wp.colors, &wp.tags);
            (wp, score)
        })
        .collect();

    // Sort by score descending
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    scored.into_iter().map(|(wp, _)| wp).collect()
}

/// Filter wallpapers that match time profile (score above threshold)
#[allow(dead_code)]
pub fn filter_by_time_profile<'a>(
    wallpapers: &'a [crate::wallpaper::Wallpaper],
    profiles: &TimeProfiles,
    min_score: f32,
) -> Vec<&'a crate::wallpaper::Wallpaper> {
    wallpapers
        .iter()
        .filter(|wp| profiles.score_wallpaper(&wp.colors, &wp.tags) >= min_score)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_period_from_hour() {
        assert_eq!(TimePeriod::from_hour(6), TimePeriod::Morning);
        assert_eq!(TimePeriod::from_hour(11), TimePeriod::Morning);
        assert_eq!(TimePeriod::from_hour(12), TimePeriod::Afternoon);
        assert_eq!(TimePeriod::from_hour(17), TimePeriod::Afternoon);
        assert_eq!(TimePeriod::from_hour(18), TimePeriod::Evening);
        assert_eq!(TimePeriod::from_hour(21), TimePeriod::Evening);
        assert_eq!(TimePeriod::from_hour(22), TimePeriod::Night);
        assert_eq!(TimePeriod::from_hour(3), TimePeriod::Night);
    }
}
