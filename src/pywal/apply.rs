use anyhow::Result;

use super::wal_cache_dir;

/// Apply exported colors (reload terminals, etc.).
pub fn apply_colors() -> Result<()> {
    use std::process::Command;

    let xres_path = wal_cache_dir().join("colors.Xresources");
    if xres_path.exists() {
        // Best effort: pywal-style Xresources reload when xrdb is available.
        let _ = Command::new("xrdb").arg("-merge").arg(&xres_path).status();
    }

    Ok(())
}
