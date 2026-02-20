use anyhow::Result;
use clap::Parser;

use super::{Cli, Commands, ProfileAction};
use crate::cli_cmds::*;
use crate::{app, init, profile, pywal, watch};

pub(crate) async fn run() -> Result<()> {
    let cli = Cli::parse();

    let config = app::Config::load()?;
    let wallpaper_dir = cli.dir.unwrap_or_else(|| config.wallpaper_dir());

    match cli.command {
        Some(Commands::Random) => {
            cmd_random(&wallpaper_dir).await?;
        }
        Some(Commands::Next) => {
            cmd_next(&wallpaper_dir).await?;
        }
        Some(Commands::Prev) => {
            cmd_prev(&wallpaper_dir).await?;
        }
        Some(Commands::Screens) => {
            cmd_screens().await?;
        }
        Some(Commands::Scan) => {
            cmd_scan(&wallpaper_dir).await?;
        }
        Some(Commands::Init) => {
            init::run_init().await?;
        }
        Some(Commands::Watch {
            interval,
            shuffle,
            watch_dir,
        }) => {
            let interval = watch::parse_interval(&interval)
                .unwrap_or_else(|| std::time::Duration::from_secs(30 * 60));
            let watch_config = watch::WatchConfig {
                interval,
                shuffle,
                watch_dir,
            };
            watch::run_watch(watch_config).await?;
        }
        Some(Commands::Profile { action }) => match action {
            ProfileAction::List => profile::cmd_profile_list()?,
            ProfileAction::Create { name } => profile::cmd_profile_create(&name)?,
            ProfileAction::Delete { name } => profile::cmd_profile_delete(&name)?,
            ProfileAction::Use { name } => profile::cmd_profile_use(&name)?,
            ProfileAction::Set { name, key, value } => {
                profile::cmd_profile_set(&name, &key, &value)?
            }
        },
        Some(Commands::Tag { action }) => {
            cmd_tag(action, &wallpaper_dir)?;
        }
        Some(Commands::Pywal { path, apply }) => {
            pywal::cmd_pywal(&path, apply)?;
        }
        Some(Commands::Pair { action }) => {
            cmd_pair(action, &wallpaper_dir)?;
        }
        #[cfg(feature = "clip")]
        Some(Commands::AutoTag {
            incremental,
            threshold,
            max_tags,
            verbose,
        }) => {
            cmd_auto_tag(&wallpaper_dir, incremental, threshold, max_tags, verbose).await?;
        }
        Some(Commands::Collection { action }) => {
            cmd_collection(action).await?;
        }
        Some(Commands::Similar { path, limit }) => {
            cmd_similar(&wallpaper_dir, &path, limit)?;
        }
        Some(Commands::TimeProfile { action }) => {
            cmd_time_profile(action, &wallpaper_dir).await?;
        }
        Some(Commands::Import { action }) => {
            cmd_import(action, &wallpaper_dir)?;
        }
        None => {
            // TUI mode
            app::run_tui(wallpaper_dir).await?;
        }
    }

    Ok(())
}
