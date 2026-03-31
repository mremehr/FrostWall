use serde::{Deserialize, Serialize};

/// pywal color scheme
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalColors {
    pub wallpaper: String,
    pub alpha: String,
    pub special: WalSpecial,
    pub colors: WalColorMap,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalSpecial {
    pub background: String,
    pub foreground: String,
    pub cursor: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WalColorMap {
    pub color0: String,
    pub color1: String,
    pub color2: String,
    pub color3: String,
    pub color4: String,
    pub color5: String,
    pub color6: String,
    pub color7: String,
    pub color8: String,
    pub color9: String,
    pub color10: String,
    pub color11: String,
    pub color12: String,
    pub color13: String,
    pub color14: String,
    pub color15: String,
}

impl WalColorMap {
    pub(crate) fn entries(&self) -> [&str; 16] {
        [
            &self.color0,
            &self.color1,
            &self.color2,
            &self.color3,
            &self.color4,
            &self.color5,
            &self.color6,
            &self.color7,
            &self.color8,
            &self.color9,
            &self.color10,
            &self.color11,
            &self.color12,
            &self.color13,
            &self.color14,
            &self.color15,
        ]
    }
}
