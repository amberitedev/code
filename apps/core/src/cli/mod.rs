//! The `copal` command line: managing Core on this machine. Servers are created and managed in
//! the App; here they are only listed, started and stopped.
//!
//! `setup`, `uninstall`, `run`, and the hidden `core`, `shim` and `store` work on this machine
//! directly; every other command talks to the running Core over the local socket (see `client`),
//! so it works the same on every OS and with the backend down. Every command draws for a
//! terminal, prints plain lines anywhere else, and prints one JSON document with `--json` (see
//! `ui`).

mod call;
pub mod client;
mod doctor;
mod home;
mod pairing;
mod servers;
mod status;
mod storage;
#[cfg(test)]
mod tests;
pub mod ui;
mod update;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use color_eyre::eyre::Result;
use serde::Serialize;
use serde_json::Value;

pub use client::Client;
use ui::Ui;

/// Manage the Amberite Core on this computer. Its Minecraft servers are made and managed in Amberite.
#[derive(Debug, Parser)]
#[command(
    name = "copal",
    bin_name = "copal",
    version,
    styles = ui::help_styles(),
    disable_help_subcommand = true,
    after_help = "Amberite Core is set up once, by its installer or with: copal setup (needs an admin)."
)]
pub struct Cli {
    /// Print one JSON document instead of text, for scripts.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Show whether Amberite Core runs, who it is paired with, and its servers.
    #[command(
        after_help = "For a script, copal list prints the servers alone, and --json prints everything."
    )]
    Status,
    /// Check everything Amberite Core needs, problems first, each with its fix.
    #[command(
        after_help = "Exits with 1 when a check fails, and with 0 when there are only warnings."
    )]
    Doctor {
        /// Apply every fix that can be applied, without asking.
        #[arg(long)]
        fix: bool,
        /// Show the facts behind every check, not only the failed ones.
        #[arg(short, long)]
        verbose: bool,
    },
    /// Pair this Amberite Core with your Amberite account.
    Pair {
        /// Show the code and leave, instead of waiting until it is approved.
        #[arg(long)]
        no_wait: bool,
    },
    /// Remove this Amberite Core from its Amberite account.
    Unpair {
        /// Do not ask first.
        #[arg(short, long)]
        yes: bool,
    },
    /// List the servers on this Amberite Core.
    #[command(
        after_help = "Piped, each server is one line of tab-separated columns: name, state, what it runs, port, who is online (empty when nobody is), id."
    )]
    List,
    /// Start a server. Without a name, pick one from a list.
    Start {
        /// The server's name, or the start of its id.
        server: Option<String>,
        /// Accept the Minecraft EULA (https://aka.ms/MinecraftEULA) for every server.
        #[arg(long)]
        accept_eula: bool,
    },
    /// Save and stop a server. Without a name, pick one from a list.
    Stop {
        /// The server's name, or the start of its id.
        server: Option<String>,
        /// Stop every running server.
        #[arg(long, conflicts_with = "server")]
        all: bool,
        /// End the server now, without saving. For one that no longer answers.
        #[arg(long)]
        kill: bool,
        /// Stop it even while players are online, without asking.
        #[arg(short, long)]
        yes: bool,
    },
    /// Show what takes up disk space: servers, backups and shared files.
    Storage {
        /// List every server and backup, not only the biggest.
        #[arg(long)]
        all: bool,
    },
    /// Show the end of Amberite Core's own log.
    #[command(
        after_help = "A terminal shows this computer's time. Piped, each line is as Amberite Core wrote it, in UTC."
    )]
    Logs {
        /// How many lines, up to 1000.
        #[arg(short = 'n', long, default_value_t = 100)]
        lines: usize,
    },
    /// Update Amberite Core, or change how it updates.
    Update(update::UpdateArgs),
    /// Remove Amberite Core from this computer. Needs an admin.
    Uninstall {
        /// Also delete the data folder: every server, world, and backup.
        #[arg(long, conflicts_with = "keep_data")]
        remove_data: bool,
        /// Keep the data folder. Without either flag, a terminal asks.
        #[arg(long)]
        keep_data: bool,
        /// Do not ask first. The data folder is kept unless --remove-data is passed.
        #[arg(short, long)]
        yes: bool,
    },
    /// Set Amberite Core up as a service on this machine. The installer runs this once, as admin.
    #[command(hide = true)]
    Setup {
        /// The account that may use the copal CLI. Defaults to the one running setup.
        #[arg(long)]
        owner: Option<String>,
        /// Set up this copal even when Amberite Core has updated itself to a newer version.
        #[arg(long)]
        allow_downgrade: bool,
    },
    /// Run Amberite Core in the foreground. The service runs this.
    #[command(hide = true)]
    Run {
        /// Set by the Windows service manager.
        #[arg(long, hide = true)]
        service: bool,
    },
    /// Amberite Core itself, as `copal run` starts it. Not for people.
    #[command(hide = true)]
    Core,
    /// Supervise one run of a server. Amberite Core starts this; it is not for people.
    #[command(hide = true)]
    Shim {
        #[arg(long)]
        data_dir: PathBuf,
        server: String,
    },
    /// The shared file pool's store. Amberite Core starts this; it is not for people.
    #[command(hide = true)]
    Store(sharing_storage::Args),
}

/// How a command ended, after it showed its output through [`Ui`].
pub struct Done {
    /// What `--json` prints.
    pub json: Value,
    /// The command did not do what was asked: the process exits non-zero.
    pub failed: bool,
}

impl Done {
    pub fn ok(json: &impl Serialize) -> Self {
        Self {
            json: serde_json::to_value(json).expect("answers serialize"),
            failed: false,
        }
    }

    pub fn failed(json: &impl Serialize) -> Self {
        Self {
            failed: true,
            ..Self::ok(json)
        }
    }
}

/// Text a server or a mod wrote, made safe to print: every control character but newline and
/// tab is shown escaped, so it cannot move the cursor, clear the screen, or change the terminal.
/// `--json` escapes by itself.
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
    let (what, next) = describe(error);
    let next = next.map(|next| format!(" {next}"));
    json_error(code, &format!("{what}{}", next.unwrap_or_default()))
}

/// Print how a command ended, and say whether it worked. An error is shown where the person is
/// looking: inside the frame a terminal has open, as one line anywhere else, as JSON with
/// `--json`.
pub fn finish(ui: &Ui, json: bool, result: Result<Done>) -> bool {
    match result {
        Ok(done) => {
            if json {
                let text = serde_json::to_string_pretty(&done.json)
                    .expect("JSON values print");
                println!("{text}");
            }
            !done.failed
        }
        Err(error) if error.is::<ui::Cancelled>() => false,
        Err(error) => {
            if json {
                println!("{}", json_for(&error));
            } else {
                let (what, next) = describe(&error);
                if !ui.abort(&what, next.as_deref()) {
                    let next = next.map(|next| format!(" {next}"));
                    eprintln!("Error: {what}{}", next.unwrap_or_default());
                }
            }
            false
        }
    }
}

/// An error as a person reads it: what is wrong, and the next step when there is one. A Core
/// that does not answer is said the way `copal` alone says it.
fn describe(error: &color_eyre::Report) -> (String, Option<String>) {
    let unreachable = error
        .is::<client::Unreachable>()
        .then(client::why_unreachable);
    match unreachable.as_ref().or_else(|| error.downcast_ref()) {
        Some(client::Stuck { what, next }) => {
            (what.clone(), Some(next.clone()))
        }
        None => (ui::sentence(&format!("{error:#}")), None),
    }
}

/// Run a command that talks to Core. False when it failed.
pub async fn run(command: Option<Command>, json: bool) -> bool {
    let ui = Ui::new(json);
    let client = Client::new();
    let result = match command {
        None => home::run(&ui, &client).await,
        Some(Command::Status) => status::run(&ui, &client).await,
        Some(Command::Doctor { fix, verbose }) => {
            doctor::run(&ui, &client, fix, verbose).await
        }
        Some(Command::Pair { no_wait }) => {
            pairing::pair(&ui, &client, no_wait).await
        }
        Some(Command::Unpair { yes }) => {
            pairing::unpair(&ui, &client, yes).await
        }
        Some(Command::List) => servers::list(&ui, &client).await,
        Some(Command::Start {
            server,
            accept_eula,
        }) => servers::start(&ui, &client, server, accept_eula).await,
        Some(Command::Stop {
            server,
            all,
            kill,
            yes,
        }) => servers::stop(&ui, &client, server, all, kill, yes).await,
        Some(Command::Storage { all }) => storage::run(&ui, &client, all).await,
        Some(Command::Logs { lines }) => {
            status::logs(&ui, &client, lines).await
        }
        Some(Command::Update(args)) => update::run(&ui, &client, args).await,
        Some(
            Command::Uninstall { .. }
            | Command::Setup { .. }
            | Command::Run { .. }
            | Command::Core
            | Command::Shim { .. }
            | Command::Store(_),
        ) => {
            unreachable!(
                "main runs the commands that do not talk to Amberite Core"
            )
        }
    };
    finish(&ui, json, result)
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
