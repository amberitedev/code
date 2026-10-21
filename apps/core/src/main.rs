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

use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

use cli::{Cli, Command};

fn main() -> ExitCode {
    // Read before parsing, so a command line clap refuses still answers in JSON.
    let json = std::env::args_os().any(|arg| arg == "--json");
    let cli = match Cli::try_parse() {
        Ok(cli) => cli,
        Err(error)
            if json
                && !matches!(
                    error.kind(),
                    ErrorKind::DisplayHelp | ErrorKind::DisplayVersion
                ) =>
        {
            println!(
                "{}",
                cli::json_error(
                    "invalid_arguments",
                    error.render().to_string().trim()
                )
            );
            return ExitCode::from(2);
        }
        Err(error) => error.exit(),
    };
    let _ = color_eyre::install();
    match run(cli) {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::FAILURE,
        Err(error) => {
            if json {
                println!("{}", cli::json_for(&error));
            } else {
                eprintln!("Error: {error:?}");
            }
            ExitCode::FAILURE
        }
    }
}

/// Run a command. False when it printed that it failed.
fn run(Cli { json, command }: Cli) -> color_eyre::Result<bool> {
    let small_runtime = || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
    };
    match command {
        Command::Install {
            owner,
            allow_downgrade,
        } => daemon::install::install(owner.as_deref(), allow_downgrade)
            .map(|()| true),
        Command::Uninstall { remove_data } => {
            daemon::install::uninstall(remove_data).map(|()| true)
        }
        Command::Run { service } => daemon::run(service).map(|()| true),
        Command::Core => daemon::core().map(|()| true),
        Command::Shim { data_dir, server } => small_runtime()?
            .block_on(servers::shim::main(data_dir, server))
            .map(|()| true),
        command => small_runtime()?.block_on(cli::run(command, json)),
    }
}
