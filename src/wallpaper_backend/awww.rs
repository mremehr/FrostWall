use super::{FillColor, ResizeMode, Transition, WallpaperBackend};
use anyhow::{Context, Result};
use std::path::Path;
use std::process::Command;

pub(super) static BACKEND: AwwwBackend = AwwwBackend;

pub(super) struct AwwwBackend;

impl WallpaperBackend for AwwwBackend {
    fn required_commands(&self) -> &'static [&'static str] {
        &["awww", "awww-daemon"]
    }

    fn set_wallpaper_with_resize(
        &self,
        output: &str,
        path: &Path,
        transition: &Transition,
        resize_mode: ResizeMode,
        fill_color: &FillColor,
    ) -> Result<()> {
        self.ensure_daemon()?;

        let output = Command::new("awww")
            .arg("img")
            .arg("-o")
            .arg(output)
            .arg(path)
            .arg("--resize")
            .arg(resize_mode.as_str())
            .arg("--fill-color")
            .arg(fill_color.to_hex())
            .arg("--transition-type")
            .arg(transition.transition_type.as_str())
            .arg("--transition-duration")
            .arg(transition.duration.to_string())
            .arg("--transition-fps")
            .arg(transition.fps.to_string())
            .output()
            .context("Failed to run awww")?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("awww failed: {}", stderr);
        }

        Ok(())
    }
}

impl AwwwBackend {
    fn ensure_daemon(&self) -> Result<()> {
        let status = Command::new("awww").arg("query").output();

        match status {
            Ok(output) if output.status.success() => Ok(()),
            _ => {
                Command::new("awww-daemon")
                    .spawn()
                    .context("Failed to start awww-daemon")?;

                std::thread::sleep(std::time::Duration::from_millis(100));
                Ok(())
            }
        }
    }
}
