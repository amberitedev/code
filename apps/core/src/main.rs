//! Copal, Amberite's self-hosted Minecraft server manager. The target design is
//! `docs/internals/core-plan.md`.

mod access;
mod api;
mod backups;
mod cli;
mod content;
mod daemon;
mod db;
mod installs;
mod network;
mod servers;
mod update;

use clap::Parser;

use cli::{Cli, Command};

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let small_runtime = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
    };
    let Cli { json, command } = Cli::parse();
    match command {
        Command::Install {
            owner,
            allow_downgrade,
        } => daemon::install::install(owner.as_deref(), allow_downgrade),
        Command::Uninstall { remove_data } => {
            daemon::install::uninstall(remove_data)
        }
        Command::Run { service } => daemon::run(service),
        Command::Core => daemon::core(),
        Command::Shim { data_dir, server } => {
            small_runtime()?.block_on(servers::shim::main(data_dir, server))
        }
        command => small_runtime()?.block_on(cli::run(command, json)),
    }
}
