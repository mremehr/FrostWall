mod app;
mod cli;
mod cli_cmds;
mod clip;
#[cfg(feature = "clip")]
mod clip_embeddings_bin;
mod collections;
mod init;
mod pairing;
mod profile;
mod pywal;
mod screen;
mod swww;
mod thumbnail;
mod timeprofile;
mod ui;
mod utils;
mod wallpaper;
mod watch;
mod webimport;

use anyhow::Result;

pub(crate) use cli::{CollectionAction, ImportAction, PairAction, TagAction, TimeProfileAction};

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
