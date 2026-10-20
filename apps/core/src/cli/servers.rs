//! `copal servers ...`. A server is named by its id, the start of its id, or its name.

use std::time::Duration;

use axum::http::Method;
use clap::{ArgGroup, Subcommand};
use color_eyre::eyre::{Result, bail};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use tokio_tungstenite::tungstenite::Message;

use super::{Client, Output, Switch, on_off};
use crate::{
    api::{
        eula::Eula,
        performance::{Memory, Performance},
        servers::{
            CreateServerRequest, CreateServerResponse, PowerAction,
            PowerRequest, ReinstallRequest, Server, ServerGetResponse,
            ServerPowerState,
        },
    },
    servers::restarts::{Schedule, TimeOfDay, Weekday},
};

#[derive(Debug, Subcommand)]
pub enum ServersCommand {
    /// List every server.
    List,
    /// Create a Vanilla server. Content and other loaders are set up from the App.
    Create {
        name: String,
        /// The Minecraft version, such as 1.21.1.
        #[arg(long)]
        version: String,
        /// Accept the Minecraft EULA (https://aka.ms/MinecraftEULA) for every server.
        #[arg(long)]
        accept_eula: bool,
    },
    /// Start a server.
    Start { server: String },
    /// Stop a server, waiting until it has saved and exited.
    Stop { server: String },
    /// Kill a server's Java process now, without saving.
    Kill { server: String },
    /// Stop a server, then start it again.
    Restart { server: String },
    /// Read a server's console or send it a command.
    #[command(subcommand)]
    Console(ConsoleCommand),
    /// Show or set the most memory a server's Java may use.
    Memory {
        server: String,
        /// In MB. The server uses it from its next start.
        mb: Option<u32>,
    },
    /// Run a server with a Java binary of your choice, or go back to the Java Core manages.
    #[command(group(ArgGroup::new("java").required(true)))]
    Java {
        server: String,
        /// The Java binary. It runs as the copal-servers account.
        #[arg(group = "java")]
        path: Option<String>,
        /// Use the Java Core manages for the server's Minecraft version.
        #[arg(long, group = "java")]
        managed: bool,
    },
    /// Show or change when a server restarts by itself.
    Restarts {
        server: String,
        /// Start the server again after a crash.
        #[arg(long)]
        crash: Option<Switch>,
        /// Restart every day at this time (HH:MM, this machine's time), with a countdown in the
        /// game.
        #[arg(long, conflicts_with = "no_schedule")]
        at: Option<TimeOfDay>,
        /// With --at: restart weekly on this day instead of daily.
        #[arg(long, requires = "at")]
        on: Option<Weekday>,
        /// No scheduled restarts.
        #[arg(long)]
        no_schedule: bool,
        /// Experimental: restart when nobody is online and the server has run this many hours.
        #[arg(long, conflicts_with = "no_smart")]
        smart: Option<u32>,
        /// No smart restarts.
        #[arg(long)]
        no_smart: bool,
    },
}

#[derive(Debug, Subcommand)]
pub enum ConsoleCommand {
    /// Follow a server's console. Lines you type are sent to the server. Ctrl+C to leave.
    Attach { server: String },
    /// Send one command to a server, such as `say hello`.
    Send {
        server: String,
        #[arg(required = true, num_args = 1.., trailing_var_arg = true)]
        command: Vec<String>,
    },
}

/// `copal servers ...`, except `console attach`.
pub async fn run(client: &Client, command: ServersCommand) -> Result<Output> {
    match command {
        ServersCommand::List => {
            let servers = list(client).await?;
            Ok(Output::new(table(&servers), &servers))
        }
        ServersCommand::Create {
            name,
            version,
            accept_eula,
        } => {
            let created: CreateServerResponse = client
                .post(
                    "/local/servers",
                    &CreateServerRequest {
                        name,
                        instance: None,
                    },
                )
                .await?;
            let id = created.id.to_string();
            client
                .post::<()>(
                    &format!("/modrinth/v0/servers/{id}/reinstall?hard=false"),
                    &ReinstallRequest::Loader {
                        loader: "vanilla".into(),
                        loader_version: None,
                        game_version: Some(version.clone()),
                    },
                )
                .await?;
            if accept_eula {
                client.post::<Eula>("/eula", &()).await?;
            }
            Ok(Output::new(
                format!(
                    "Created server {id} with Minecraft {version}. Start it with: copal servers start {id}"
                ),
                &created,
            ))
        }
        ServersCommand::Start { server } => {
            let server = find(client, &server).await?;
            power(client, &server, PowerAction::Start).await?;
            done(client, &server, "is running").await
        }
        ServersCommand::Stop { server } => {
            let server = find(client, &server).await?;
            power(client, &server, PowerAction::Stop).await?;
            eprintln!("Stopping {}...", server.name);
            wait_offline(client, &server).await
        }
        ServersCommand::Kill { server } => {
            let server = find(client, &server).await?;
            power(client, &server, PowerAction::Kill).await?;
            wait_offline(client, &server).await
        }
        ServersCommand::Restart { server } => {
            let server = find(client, &server).await?;
            power(client, &server, PowerAction::Restart).await?;
            done(client, &server, "is restarting").await
        }
        ServersCommand::Console(ConsoleCommand::Send { server, command }) => {
            let server = find(client, &server).await?;
            let command = command.join(" ");
            let mut console =
                client.console(&server.server_id.to_string()).await?;
            console
                .send(Message::text(
                    serde_json::json!({ "event": "command", "cmd": command })
                        .to_string(),
                ))
                .await?;
            console.close(None).await?;
            Ok(Output::new(
                format!("Sent to {}: {command}", server.name),
                &serde_json::json!({ "server_id": server.server_id, "command": command }),
            ))
        }
        ServersCommand::Console(ConsoleCommand::Attach { .. }) => {
            unreachable!("attach streams; see attach")
        }
        ServersCommand::Memory { server, mb } => {
            let server = find(client, &server).await?;
            let path = format!("/servers/{}/memory", server.server_id);
            let performance: Performance = match mb {
                Some(memory_mb) => {
                    client
                        .call(Method::PUT, &path, Some(&Memory { memory_mb }))
                        .await?
                }
                None => client.get(&performance_path(&server)).await?,
            };
            let mut text =
                format!("{}: {} MB", server.name, performance.memory_mb);
            if mb.is_some() && server.power_state.is_some_and(is_up) {
                text.push_str(", from its next start");
            }
            Ok(Output::new(text, &performance))
        }
        ServersCommand::Java {
            server,
            path,
            managed: _,
        } => {
            let server = find(client, &server).await?;
            let body = JavaPath { path: path.clone() };
            client
                .call::<()>(
                    Method::PUT,
                    &format!("/servers/{}/java", server.server_id),
                    Some(&body),
                )
                .await?;
            let text = match path {
                Some(path) => format!("{} runs {path}", server.name),
                None => format!("{} runs the Java Core manages", server.name),
            };
            Ok(Output::new(text, &body))
        }
        ServersCommand::Restarts {
            server,
            crash,
            at,
            on,
            no_schedule,
            smart,
            no_smart,
        } => {
            let server = find(client, &server).await?;
            let mut performance: Performance =
                client.get(&performance_path(&server)).await?;
            let restarts = &mut performance.restarts;
            let before = restarts.clone();
            if let Some(crash) = crash {
                restarts.crash = crash.into();
            }
            if let Some(at) = at {
                restarts.schedule = Some(match on {
                    Some(on) => Schedule::Weekly { on, at },
                    None => Schedule::Daily { at },
                });
            }
            if no_schedule {
                restarts.schedule = None;
            }
            if smart.is_some() {
                restarts.smart_after_hours = smart;
            }
            if no_smart {
                restarts.smart_after_hours = None;
            }
            if *restarts != before {
                performance = client
                    .call(
                        Method::PUT,
                        &format!("/servers/{}/restarts", server.server_id),
                        Some(&*restarts),
                    )
                    .await?;
            }
            let restarts = &performance.restarts;
            let schedule = match restarts.schedule {
                None => "none".to_owned(),
                Some(Schedule::Daily { at }) => {
                    format!("daily at {}", String::from(at))
                }
                Some(Schedule::Weekly { on, at }) => format!(
                    "{} at {}",
                    format!("{on:?}").to_lowercase(),
                    String::from(at)
                ),
            };
            let smart = restarts
                .smart_after_hours
                .map_or("off".to_owned(), |hours| {
                    format!("when empty after {hours} hours")
                });
            Ok(Output::new(
                format!(
                    "{}\n  after a crash: {}\n  scheduled: {schedule}\n  smart: {smart}",
                    server.name,
                    on_off(restarts.crash)
                ),
                &performance,
            ))
        }
    }
}

/// The body of `PUT /servers/{id}/java`, as step 4's `api::startup::JavaPath` has it.
#[derive(Debug, Serialize)]
struct JavaPath {
    path: Option<String>,
}

fn performance_path(server: &Server) -> String {
    format!("/servers/{}/performance", server.server_id)
}

/// Every server.
pub async fn list(client: &Client) -> Result<Vec<Server>> {
    let response: ServerGetResponse =
        client.get("/modrinth/v0/servers").await?;
    Ok(response.servers)
}

/// One line per server.
pub fn table(servers: &[Server]) -> String {
    if servers.is_empty() {
        return "No servers yet. Create one with: copal servers create <name> --version <version>".into();
    }
    servers
        .iter()
        .map(|server| {
            format!(
                "{}  {:<9} {:<10} port {:<5}  {}",
                server.server_id,
                server.power_state.map_or("unknown", state_name),
                server.mc_version.as_deref().unwrap_or("-"),
                server.net.port,
                server.name,
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// The server `query` names: its id, the unique start of its id, or its name.
pub async fn find(client: &Client, query: &str) -> Result<Server> {
    pick(list(client).await?, query)
}

fn pick(servers: Vec<Server>, query: &str) -> Result<Server> {
    let query_lower = query.to_lowercase();
    let mut matches: Vec<Server> = servers
        .into_iter()
        .filter(|server| {
            let id = server.server_id.to_string();
            id.starts_with(&query_lower)
                || server.name.to_lowercase() == query_lower
        })
        .collect();
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => bail!("there is no server {query}. See copal servers list"),
        _ => bail!(
            "{query} names more than one server; use more of its id. See copal servers list"
        ),
    }
}

fn state_name(state: ServerPowerState) -> &'static str {
    match state {
        ServerPowerState::Offline => "stopped",
        ServerPowerState::Starting => "starting",
        ServerPowerState::Running => "running",
        ServerPowerState::Stopping => "stopping",
        ServerPowerState::Crashed => "crashed",
    }
}

fn is_up(state: ServerPowerState) -> bool {
    matches!(
        state,
        ServerPowerState::Starting
            | ServerPowerState::Running
            | ServerPowerState::Stopping
    )
}

async fn power(
    client: &Client,
    server: &Server,
    action: PowerAction,
) -> Result<()> {
    client
        .post(
            &format!("/modrinth/v0/servers/{}/power", server.server_id),
            &PowerRequest { action },
        )
        .await
}

/// The server as it is now, with `what` said about it.
async fn done(client: &Client, server: &Server, what: &str) -> Result<Output> {
    let server: Server = client
        .get(&format!("/modrinth/v0/servers/{}", server.server_id))
        .await?;
    Ok(Output::new(format!("{} {what}", server.name), &server))
}

/// Wait, with no time limit, until the server's Java has exited.
async fn wait_offline(client: &Client, server: &Server) -> Result<Output> {
    loop {
        let now: Server = client
            .get(&format!("/modrinth/v0/servers/{}", server.server_id))
            .await?;
        match now.power_state {
            Some(state) if !is_up(state) => {
                return Ok(Output::new(
                    format!("{} is {}", now.name, state_name(state)),
                    &now,
                ));
            }
            _ => tokio::time::sleep(Duration::from_millis(500)).await,
        }
    }
}

/// A console event, with only what the CLI shows.
#[derive(Debug, Deserialize)]
struct Event {
    event: String,
    message: Option<String>,
}

/// `copal servers console attach`: print the console as it comes, and send each line typed.
/// With `--json`, each event is printed as Core sent it.
pub async fn attach(client: &Client, server: &str, json: bool) -> Result<()> {
    let server = find(client, server).await?;
    let mut console = client.console(&server.server_id.to_string()).await?;
    eprintln!(
        "Attached to {}. Lines you type go to the server; Ctrl+C leaves it running.",
        server.name
    );
    let (typed, mut lines) = tokio::sync::mpsc::channel::<String>(16);
    // A thread of its own, so a pending read never holds the process open.
    std::thread::spawn(move || {
        for line in std::io::stdin().lines().map_while(Result::ok) {
            if typed.blocking_send(line).is_err() {
                return;
            }
        }
    });
    loop {
        tokio::select! {
            message = console.next() => match message {
                Some(Ok(Message::Text(text))) => {
                    if json {
                        println!("{text}");
                    } else if let Ok(Event { event, message: Some(message) }) =
                        serde_json::from_str(&text)
                        && event == "log"
                    {
                        println!("{message}");
                    }
                }
                Some(Ok(Message::Close(_))) | None => {
                    eprintln!("The console closed.");
                    return Ok(());
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error.into()),
            },
            Some(line) = lines.recv() => {
                console
                    .send(Message::text(
                        serde_json::json!({ "event": "command", "cmd": line }).to_string(),
                    ))
                    .await?;
            }
            _ = tokio::signal::ctrl_c() => return Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn server(id: &str, name: &str) -> Server {
        let mut server: Server = serde_json::from_value(serde_json::json!({
            "server_id": id, "name": name, "owner_id": "", "net": {"ip": null, "port": 25565, "domain": "", "outside": null},
            "game": "Minecraft", "backup_quota": 0, "used_backup_quota": 0, "status": "available",
            "suspension_reason": null, "loader": null, "loader_version": null, "mc_version": null,
            "upstream": null, "sftp_username": "", "sftp_password": "", "sftp_host": "",
            "datacenter": "", "notices": [], "node": null, "flows": {"intro": false}, "is_medal": false,
            "current_user_permissions": 0, "locked_since": null, "linked_instance": null,
            "reachability": null
        }))
        .unwrap();
        server.name = name.into();
        server
    }

    #[test]
    fn a_server_is_named_by_id_start_or_name() {
        let servers = || {
            vec![
                server("a1b2c3d4-0000-4000-8000-000000000001", "Survival"),
                server("a1b2ffff-0000-4000-8000-000000000002", "Creative"),
            ]
        };
        assert_eq!(pick(servers(), "a1b2c").unwrap().name, "Survival");
        assert_eq!(pick(servers(), "creative").unwrap().name, "Creative");
        assert!(pick(servers(), "a1b2").is_err(), "ambiguous");
        assert!(pick(servers(), "nope").is_err());
    }
}
