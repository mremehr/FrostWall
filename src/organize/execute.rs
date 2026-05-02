use super::plan::PlannedRename;
use anyhow::{Context, Result};
use std::fs;
use std::path::PathBuf;

struct RenameStage {
    original: PathBuf,
    temp: PathBuf,
    final_path: PathBuf,
}

pub(super) fn execute_plan(plan: &[PlannedRename]) -> Result<()> {
    if plan.is_empty() {
        return Ok(());
    }

    let mut stages = Vec::with_capacity(plan.len());
    let pid = std::process::id();

    for (idx, rename) in plan.iter().enumerate() {
        let parent = rename
            .from
            .parent()
            .context("Wallpaper path had no parent directory")?;
        let extension = rename
            .from
            .extension()
            .and_then(|ext| ext.to_str())
            .unwrap_or("tmp");
        let mut temp_path = parent.join(format!(".frostwall-rename-{pid}-{idx}.{extension}"));
        let mut suffix = 0_u32;
        while temp_path.exists() {
            suffix += 1;
            temp_path = parent.join(format!(
                ".frostwall-rename-{pid}-{idx}-{suffix}.{extension}"
            ));
        }

        stages.push(RenameStage {
            original: rename.from.clone(),
            temp: temp_path,
            final_path: rename.to.clone(),
        });
    }

    let mut phase_one_complete = 0;
    for stage in &stages {
        if let Err(error) = fs::rename(&stage.original, &stage.temp) {
            rollback_stages(&stages, phase_one_complete, 0);
            return Err(error).with_context(|| {
                format!(
                    "Failed to move {} to temporary path",
                    stage.original.display()
                )
            });
        }
        phase_one_complete += 1;
    }

    for (finalized, stage) in stages.iter().enumerate() {
        if let Err(error) = fs::rename(&stage.temp, &stage.final_path) {
            rollback_stages(&stages, phase_one_complete, finalized);
            return Err(error).with_context(|| {
                format!(
                    "Failed to finalize rename {} -> {}",
                    stage.original.display(),
                    stage.final_path.display()
                )
            });
        }
    }

    Ok(())
}

fn rollback_stages(stages: &[RenameStage], phase_one_complete: usize, finalized: usize) {
    for stage in stages[..finalized].iter().rev() {
        if stage.final_path.exists() {
            let _ = fs::rename(&stage.final_path, &stage.original);
        }
    }

    for stage in stages[finalized..phase_one_complete].iter().rev() {
        if stage.temp.exists() {
            let _ = fs::rename(&stage.temp, &stage.original);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::{ImageBuffer, Rgba};
    use std::path::Path;

    fn write_png(path: &Path, width: u32, height: u32) {
        let image = ImageBuffer::from_pixel(width, height, Rgba([32_u8, 64, 96, 255]));
        image.save(path).unwrap();
    }

    #[test]
    fn execute_plan_renames_files_in_place() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("source.png");
        let to = dir.path().join("landscape-wallpaper1.png");
        write_png(&from, 1920, 1080);

        execute_plan(&[PlannedRename {
            from: from.clone(),
            to: to.clone(),
        }])
        .unwrap();

        assert!(!from.exists());
        assert!(to.exists());
    }
}
