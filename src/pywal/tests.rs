use std::fs;
use std::path::Path;

use anyhow::Result;
use tempfile::tempdir;

use super::{export::export_colors_to, generate_palette, WalColorMap, WalColors, WalSpecial};

#[test]
fn generate_palette_fills_missing_base_colors() {
    let wallpaper_path = Path::new("/tmp/fallback-wallpaper.png");
    let palette = generate_palette(&["#111111".to_string()], wallpaper_path);

    assert_eq!(palette.wallpaper, wallpaper_path.to_string_lossy());
    assert_eq!(palette.alpha, "100");
    assert_eq!(palette.colors.color1, "#111111");
    assert_eq!(palette.colors.color5, "#808080");
    assert_eq!(palette.special.background, palette.colors.color0);
    assert_eq!(palette.special.foreground, palette.colors.color7);
}

#[test]
fn export_colors_to_writes_expected_pywal_files() -> Result<()> {
    let dir = tempdir()?;
    let colors = sample_colors();

    export_colors_to(dir.path(), &colors)?;

    let json = fs::read_to_string(dir.path().join("colors.json"))?;
    assert!(json.contains("\"wallpaper\": \"/tmp/it'wallpaper.jpg\""));

    let plain = fs::read_to_string(dir.path().join("colors"))?;
    assert_eq!(plain.lines().count(), 16);
    assert!(plain.ends_with('\n'));

    let shell = fs::read_to_string(dir.path().join("colors.sh"))?;
    assert!(shell.contains("wallpaper='/tmp/it'\\''wallpaper.jpg'"));
    assert!(shell.contains("color15='#ffffff'"));

    let xresources = fs::read_to_string(dir.path().join("colors.Xresources"))?;
    assert!(xresources.contains("*background: #101010"));
    assert!(xresources.contains("*color14: #eeeeee"));

    Ok(())
}

fn sample_colors() -> WalColors {
    WalColors {
        wallpaper: "/tmp/it'wallpaper.jpg".to_string(),
        alpha: "100".to_string(),
        special: WalSpecial {
            background: "#101010".to_string(),
            foreground: "#f0f0f0".to_string(),
            cursor: "#f0f0f0".to_string(),
        },
        colors: WalColorMap {
            color0: "#101010".to_string(),
            color1: "#202020".to_string(),
            color2: "#303030".to_string(),
            color3: "#404040".to_string(),
            color4: "#505050".to_string(),
            color5: "#606060".to_string(),
            color6: "#707070".to_string(),
            color7: "#808080".to_string(),
            color8: "#909090".to_string(),
            color9: "#a0a0a0".to_string(),
            color10: "#b0b0b0".to_string(),
            color11: "#c0c0c0".to_string(),
            color12: "#d0d0d0".to_string(),
            color13: "#e0e0e0".to_string(),
            color14: "#eeeeee".to_string(),
            color15: "#ffffff".to_string(),
        },
    }
}
