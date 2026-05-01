mod app;
mod cli;
mod cli_cmds;
mod clip;
#[cfg(feature = "clip")]
mod clip_embeddings_bin;
mod collections;
mod init;
mod organize;
mod pairing;
mod profile;
mod pywal;
mod screen;
mod thumbnail;
mod timeprofile;
mod ui;
mod utils;
mod wallpaper;
mod wallpaper_backend;
mod watch;
mod webimport;

use anyhow::Result;

pub(crate) use cli::{
    CollectionAction, ImportAction, OrganizeAction, PairAction, TagAction, TimeProfileAction,
};

#[tokio::main]
async fn main() -> Result<()> {
    cli::run().await
}
