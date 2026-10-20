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

use std::path::PathBuf;

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
    /// Install Copal as a service on this machine. Run once, as admin.
    Install {
        /// The account that may use the copal CLI. Defaults to the one running install.
        #[arg(long)]
        owner: Option<String>,
    },
    /// Remove the service, the accounts, and the firewall rules. Run as admin.
    Uninstall {
        /// Also delete the data folder: every server, world, and backup.
        #[arg(long)]
        remove_data: bool,
    },
    /// Run Core in the foreground.
    Run {
        /// Set by the Windows service manager.
        #[arg(long, hide = true)]
        service: bool,
    },
    /// Core itself, as `copal run` starts it. Not for people.
    #[command(hide = true)]
    Core,
    /// Show whether Core runs and what its servers are doing.
    Status,
    /// Create and control servers.
    #[command(subcommand)]
    Servers(cli::ServersCommand),
    /// Supervise one run of a server. Core starts this; it is not for people.
    #[command(hide = true)]
    Shim {
        #[arg(long)]
        data_dir: PathBuf,
        server: String,
    },
}

fn main() -> color_eyre::Result<()> {
    color_eyre::install()?;
    let small_runtime = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
    };
    match Cli::parse().command {
        Command::Install { owner } => {
            daemon::install::install(owner.as_deref())
        }
        Command::Uninstall { remove_data } => {
            daemon::install::uninstall(remove_data)
        }
        Command::Run { service } => daemon::run(service),
        Command::Core => daemon::core(),
        Command::Status => small_runtime()?.block_on(cli::status()),
        Command::Servers(command) => {
            small_runtime()?.block_on(cli::servers(command))
        }
        Command::Shim { data_dir, server } => {
            small_runtime()?.block_on(servers::shim::main(data_dir, server))
        }
    }
}
