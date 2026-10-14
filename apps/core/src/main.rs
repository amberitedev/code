//! Copal, Amberite's self-hosted Minecraft server manager. The target design is
//! `docs/internals/core-plan.md`.

mod access;
mod api;
mod backups;
mod content;
mod daemon;
mod db;
mod installs;
mod network;
mod servers;
mod update;

use clap::{Parser, Subcommand};

/// Copal runs and manages Minecraft servers on this machine.
#[derive(Parser)]
#[command(name = "copal", version)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Run Core in the foreground.
    Run,
}

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    match Cli::parse().command {
        Command::Run => daemon::run().await,
    }
}
