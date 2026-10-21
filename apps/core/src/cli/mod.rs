//! The `copal` command line. `install`, `uninstall`, `run`, and the hidden `core` and `shim` work
//! on this machine directly; every other command talks to the running Core over the local socket
//! (see `client`), so it works the same on every OS and with the backend down.
//!
//! The CLI is for the machine and for controlling servers. Content (mods, packs) stays in the
//! App. Every command prints text for a person, or with `--json` the JSON Core answered.

mod access;
mod backups;
pub mod client;
mod machine;
mod servers;
#[cfg(test)]
mod tests;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use serde::Serialize;
use serde_json::Value;

pub use client::Client;

/// Copal runs and manages Minecraft servers on this machine.
#[derive(Debug, Parser)]
#[command(name = "copal", version)]
pub struct Cli {
    /// Print the JSON Core answered, for scripts.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Install Copal as a service on this machine. Run once, as admin.
    Install {
        /// The account that may use the copal CLI. Defaults to the one running install.
        #[arg(long)]
        owner: Option<String>,
        /// Install this copal even when Core has updated itself to a newer version.
        #[arg(long)]
        allow_downgrade: bool,
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
    /// Supervise one run of a server. Core starts this; it is not for people.
    #[command(hide = true)]
    Shim {
        #[arg(long)]
        data_dir: PathBuf,
        server: String,
    },
    /// Show whether Core runs, its version, and what its servers are doing.
    Status,
    /// Show the end of Core's own log.
    Logs {
        /// How many lines, up to 1000.
        #[arg(short = 'n', long, default_value_t = 100)]
        lines: usize,
    },
    /// Update Core, or change how it updates.
    #[command(subcommand)]
    Update(machine::UpdateCommand),
    /// Show or accept the Minecraft EULA, once for every server on this Core.
    Eula {
        /// Accept the EULA (https://aka.ms/MinecraftEULA).
        #[arg(long)]
        accept: bool,
    },
    /// Create and control servers.
    #[command(subcommand)]
    Servers(servers::ServersCommand),
    /// List, make, and restore backups.
    #[command(subcommand)]
    Backups(backups::BackupsCommand),
    /// Who may use each server, and their sessions.
    #[command(subcommand)]
    Access(access::AccessCommand),
}

/// What a command prints: `text` for a person, `json` with `--json`.
pub struct Output {
    pub text: String,
    pub json: Value,
    /// The command did not do what was asked. It still prints, then exits non-zero.
    pub failed: bool,
}

impl Output {
    pub fn new(text: impl Into<String>, json: &impl Serialize) -> Self {
        Self {
            text: text.into(),
            json: serde_json::to_value(json).expect("answers serialize"),
            failed: false,
        }
    }

    pub fn failed(text: impl Into<String>, json: &impl Serialize) -> Self {
        Self {
            failed: true,
            ..Self::new(text, json)
        }
    }
}

/// Text a server or a mod wrote, made safe to print: every control character but newline and
/// tab is shown escaped, so console output cannot move the cursor, clear the screen, or change
/// the terminal. `--json` escapes by itself.
pub fn safe(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                c.escape_unicode().to_string()
            } else {
                c.to_string()
            }
        })
        .collect()
}

/// An error as `--json` prints it, in the shape of Core's own errors: `error` is a code and
/// `description` is ready to show. Core's code when Core refused, `core_unreachable` when it
/// could not be reached, `invalid_arguments` for the command line, `failed` otherwise.
pub fn json_error(code: &str, description: &str) -> Value {
    serde_json::json!({ "error": code, "description": description })
}

/// The `--json` form of an error a command returned.
pub fn json_for(error: &color_eyre::Report) -> Value {
    if let Some(client::Refused(refused)) = error.downcast_ref() {
        let code = serde_json::to_value(refused.error)
            .ok()
            .and_then(|code| code.as_str().map(str::to_owned))
            .unwrap_or_else(|| "failed".into());
        return json_error(&code, &refused.description);
    }
    let code = if error.downcast_ref::<client::Unreachable>().is_some() {
        "core_unreachable"
    } else {
        "failed"
    };
    json_error(code, &format!("{error:#}"))
}

/// Run a command that talks to Core, and print its output. False when the command printed
/// that it failed.
pub async fn run(command: Command, json: bool) -> Result<bool> {
    let client = Client::new();
    let output = match command {
        Command::Status => machine::status(&client).await?,
        Command::Logs { lines } => machine::logs(&client, lines).await?,
        Command::Update(command) => machine::update(&client, command).await?,
        Command::Eula { accept } => machine::eula(&client, accept).await?,
        Command::Servers(servers::ServersCommand::Console(
            servers::ConsoleCommand::Attach { server },
        )) => {
            return servers::attach(&client, &server, json)
                .await
                .map(|()| true);
        }
        Command::Servers(command) => servers::run(&client, command).await?,
        Command::Backups(command) => backups::run(&client, command).await?,
        Command::Access(command) => access::run(&client, command).await?,
        Command::Install { .. }
        | Command::Uninstall { .. }
        | Command::Run { .. }
        | Command::Core
        | Command::Shim { .. } => {
            unreachable!("main runs the commands that do not talk to Core")
        }
    };
    if json {
        println!("{}", serde_json::to_string_pretty(&output.json)?);
    } else if output.failed {
        eprintln!("{}", output.text);
    } else if !output.text.is_empty() {
        println!("{}", output.text);
    }
    Ok(!output.failed)
}

/// `on` or `off`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum Switch {
    On,
    Off,
}

impl From<Switch> for bool {
    fn from(switch: Switch) -> Self {
        switch == Switch::On
    }
}

/// `true` as `on`, for showing settings.
fn on_off(on: bool) -> &'static str {
    if on { "on" } else { "off" }
}

/// A unix time as the machine's local date and time.
fn local_time(unix: i64) -> String {
    chrono::DateTime::from_timestamp(unix, 0).map_or_else(
        || unix.to_string(),
        |time| {
            time.with_timezone(&chrono::Local)
                .format("%Y-%m-%d %H:%M")
                .to_string()
        },
    )
}
