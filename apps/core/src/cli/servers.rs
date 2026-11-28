//! `copal list`, `copal start` and `copal stop`. A server is named by its name, or the start of
//! its id. Everything else about a server is done in the App.

use std::time::Duration;

use color_eyre::eyre::{Result, bail};
use contracts::{
    Empty, Json,
    core::{
        AcceptEula, ChangePower, GetEula, GetServer, ListServers, PowerAction,
        PowerChange, Server, ServerId, ServerPower,
    },
};

use super::{
    Client, Done,
    call::call,
    ui::{self, Mark, Ui},
};

/// `copal list`.
pub async fn list(ui: &Ui, client: &Client) -> Result<Done> {
    ui.intro("copal list");
    let servers = all(client).await?;
    if servers.is_empty() {
        ui.outro(NO_SERVERS);
        return Ok(Done::ok(&servers));
    }
    if ui.terminal() {
        ui.say(
            Mark::Done,
            format!("{}\n{}", count(&servers), table(&servers, true)),
        );
        let up = servers
            .iter()
            .filter(|server| is_up(server.power_state))
            .count();
        ui.outro(if up == 0 {
            "Start one with: copal start"
        } else if up == servers.len() {
            "Stop one with: copal stop"
        } else {
            "Start or stop one with: copal start, copal stop"
        });
    } else {
        ui.say(Mark::Done, table(&servers, false));
    }
    Ok(Done::ok(&servers))
}

pub const NO_SERVERS: &str = "No servers yet. Create one in Amberite.";

/// `2 servers, 1 running`.
pub fn count(servers: &[Server]) -> String {
    let running = servers
        .iter()
        .filter(|server| is_up(server.power_state))
        .count();
    let total = servers.len();
    format!(
        "{total} server{}, {running} running",
        if total == 1 { "" } else { "s" }
    )
}

/// One line per server: name, state, what it runs, its port, and who is online. `drawn` is for a
/// terminal: a heading, and a coloured dot for each state. Without it the columns are
/// separated by tabs and end with the id, whose start names the server on the command line.
pub fn table(servers: &[Server], drawn: bool) -> String {
    let width = |title: &str, of: fn(&Server) -> String| {
        servers
            .iter()
            .map(|server| of(server).chars().count())
            .chain([title.len()])
            .max()
            .unwrap_or(0)
    };
    let name_width = width("NAME", |server| super::safe(&server.name));
    let runs_width = width("RUNS", runs);
    let mut lines = Vec::new();
    if drawn {
        lines.push(ui::dim(format!(
            "  {:<name_width$}  {:<8}  {:<runs_width$}  PORT   ONLINE",
            "NAME", "STATE", "RUNS"
        )));
    }
    lines.extend(servers.iter().map(|server| {
        let name = super::safe(&server.name);
        let state = state_name(server.power_state);
        let runs = runs(server);
        let port = server.net.game_port;
        if drawn {
            let online = match server.players.as_deref() {
                None => String::new(),
                Some([]) => ui::dim("nobody"),
                // A full server's names would run off the screen.
                Some([first, second, _, rest @ ..]) => super::safe(&format!(
                    "{first}, {second} +{}",
                    rest.len() + 1
                )),
                Some(players) => super::safe(&players.join(", ")),
            };
            format!(
                "{} {name:<name_width$}  {state:<8}  {runs:<runs_width$}  {port:<5}  {online}",
                dot(server.power_state),
            )
        } else {
            // Tabs, so a name or a loader with a space in it is still one column.
            [
                name.as_str(),
                state,
                runs.as_str(),
                &port.to_string(),
                &super::safe(
                    &server.players.as_deref().unwrap_or_default().join(","),
                ),
                server.server_id.as_str(),
            ]
            .join("\t")
        }
    }));
    lines.join("\n")
}

/// What a server runs: `Fabric 1.21.1`.
fn runs(server: &Server) -> String {
    match (&server.loader, &server.mc_version) {
        (Some(loader), Some(version)) => format!("{loader:?} {version}"),
        (None, Some(version)) => version.clone(),
        _ => "not set up".into(),
    }
}

fn dot(state: ServerPower) -> String {
    let on = ui::glyph("●", "*");
    match state {
        ServerPower::Running => ui::green(on),
        ServerPower::Starting | ServerPower::Stopping => ui::yellow(on),
        ServerPower::Crashed => ui::red(ui::glyph("✖", "x")),
        // The same dot, dim: a hollow one is what a list to pick from marks its rows with.
        ServerPower::Stopped | ServerPower::Killed => {
            ui::dim(ui::glyph("●", "-"))
        }
    }
}

/// `copal start`.
pub async fn start(
    ui: &Ui,
    client: &Client,
    server: Option<String>,
    accept_eula: bool,
) -> Result<Done> {
    ui.intro("copal start");
    let servers = all(client).await?;
    if servers.is_empty() {
        ui.outro(NO_SERVERS);
        return Ok(Done::failed(&servers));
    }
    let server = match server {
        Some(query) => pick(servers, &query)?,
        None => {
            let stopped: Vec<Server> = servers
                .into_iter()
                .filter(|server| !is_up(server.power_state))
                .collect();
            if stopped.is_empty() {
                ui.outro("Every server is already running.");
                return Ok(Done::ok(&stopped));
            }
            choose(ui, "Which server do you want to start?", stopped, "start")?
        }
    };
    if is_up(server.power_state) {
        ui.outro(format!(
            "{} is already {}.",
            super::safe(&server.name),
            state_name(server.power_state)
        ));
        return Ok(Done::ok(&server));
    }
    if !server.eula_accepted {
        accept(ui, client, accept_eula).await?;
    }
    let name = super::safe(&server.name);
    let spinner = ui.spinner(format!("Starting {name}"));
    if let Err(error) = power(client, &server, PowerAction::Start).await {
        spinner.clear();
        return Err(error.wrap_err(format!("{name} did not start")));
    }
    let server = match call::<GetServer>(client, &id(&server), &(), Empty).await
    {
        Ok(server) => server,
        Err(error) => {
            spinner.clear();
            return Err(error);
        }
    };
    spinner.done(format!("Started {name} on port {}", server.net.game_port));
    if ui.terminal() {
        ui.outro("Its world is still loading. Check on it with: copal status");
    }
    Ok(Done::ok(&server))
}

/// No server starts before the owner accepts the Minecraft EULA, once for every server.
async fn accept(ui: &Ui, client: &Client, flag: bool) -> Result<()> {
    if !flag {
        let eula = call::<GetEula>(client, &(), &(), Empty).await?;
        if !ui.terminal() {
            bail!(
                "servers start once you accept the Minecraft EULA ({}). Pass --accept-eula to accept it, or accept it in Amberite",
                eula.url
            );
        }
        ui.say(
            Mark::Info,
            format!(
                "Minecraft servers need you to accept the Minecraft EULA first.\n{}",
                eula.url
            ),
        );
        let question =
            "Do you accept the Minecraft EULA, for every server on this Core?";
        if !ui.confirm(question, "--accept-eula")? {
            bail!(
                "the server was not started: the Minecraft EULA is not accepted"
            );
        }
    }
    call::<AcceptEula>(client, &(), &(), Empty).await?;
    Ok(())
}

/// `copal stop`.
pub async fn stop(
    ui: &Ui,
    client: &Client,
    server: Option<String>,
    every: bool,
    kill: bool,
    yes: bool,
) -> Result<Done> {
    ui.intro("copal stop");
    let servers = all(client).await?;
    let running = |servers: Vec<Server>| -> Vec<Server> {
        servers
            .into_iter()
            .filter(|server| is_up(server.power_state))
            .collect()
    };
    let chosen = match server {
        Some(query) => {
            let server = pick(servers, &query)?;
            if !is_up(server.power_state) {
                ui.outro(format!(
                    "{} is already stopped.",
                    super::safe(&server.name)
                ));
                return Ok(Done::ok(&server));
            }
            vec![server]
        }
        None => {
            let running = running(servers);
            if running.is_empty() {
                ui.outro("No server is running.");
                return Ok(Done::ok(&running));
            }
            if every {
                running
            } else {
                vec![choose(
                    ui,
                    "Which server do you want to stop?",
                    running,
                    "stop",
                )?]
            }
        }
    };
    let (action, doing) = if kill {
        (PowerAction::Kill, "Ending")
    } else {
        (PowerAction::Stop, "Stopping")
    };
    // Stopping throws the players out, so someone has to mean it: asked of every server before
    // any is stopped, so a no, or nobody to ask, leaves them all as they were.
    let mut confirmed = Vec::new();
    for server in &chosen {
        let name = super::safe(&server.name);
        if let Some(players) = server.players.as_deref()
            && !players.is_empty()
            && !yes
        {
            let question = format!(
                "{} on {name} now: {}. Stop it anyway?",
                match players.len() {
                    1 => "1 player is".to_owned(),
                    count => format!("{count} players are"),
                },
                super::safe(&players.join(", "))
            );
            if !ui.confirm(&question, "--yes")? {
                continue;
            }
        }
        confirmed.push(server);
    }
    let mut stopped = Vec::new();
    if confirmed.is_empty() {
        ui.outro(match chosen.as_slice() {
            [server] => {
                format!("{} keeps running.", super::safe(&server.name))
            }
            _ => "Nothing was stopped.".to_owned(),
        });
        return Ok(Done::ok(&stopped));
    }
    for server in confirmed {
        let name = super::safe(&server.name);
        let spinner = ui.spinner(if kill {
            format!("{doing} {name} without saving")
        } else {
            format!("Saving and stopping {name}")
        });
        if let Err(error) = power(client, server, action).await {
            spinner.clear();
            return Err(error.wrap_err(format!("{name} did not stop")));
        }
        let now = match wait_offline(client, server).await {
            Ok(now) => now,
            Err(error) => {
                spinner.clear();
                return Err(error);
            }
        };
        spinner.done(if kill {
            format!("Ended {name} without saving")
        } else {
            format!("Stopped {name}")
        });
        stopped.push(now);
    }
    let kept: Vec<String> = chosen
        .iter()
        .filter(|server| {
            !stopped
                .iter()
                .any(|stopped: &Server| stopped.server_id == server.server_id)
        })
        .map(|server| super::safe(&server.name))
        .collect();
    if !kept.is_empty() {
        ui.say(Mark::Info, format!("Still running: {}", kept.join(", ")));
    }
    if ui.terminal() {
        ui.outro("Start it again with: copal start");
    }
    Ok(match (every, stopped.as_slice()) {
        (false, [server]) => Done::ok(server),
        _ => Done::ok(&stopped),
    })
}

/// Pick one of `servers` from a list. Outside a terminal there is nobody to pick, so the
/// answer names them.
fn choose(
    ui: &Ui,
    question: &str,
    servers: Vec<Server>,
    command: &str,
) -> Result<Server> {
    let items: Vec<(usize, String, String)> = servers
        .iter()
        .enumerate()
        .map(|(index, server)| {
            // What tells two servers apart is on every row, not only the one in focus.
            (
                index,
                format!(
                    "{}  {}",
                    super::safe(&server.name),
                    ui::dim(format!(
                        "{}, port {}{}",
                        runs(server),
                        server.net.game_port,
                        match server.players.as_deref() {
                            None => String::new(),
                            Some([]) => ", nobody online".into(),
                            Some(players) =>
                                format!(", {} online", players.len()),
                        }
                    ))
                ),
                String::new(),
            )
        })
        .collect();
    let names: Vec<String> = servers
        .iter()
        .map(|server| super::safe(&server.name))
        .collect();
    let how = if command == "stop" {
        format!(
            "Name the server: copal stop <server>, or stop them all: copal stop --all. Running: {}",
            names.join(", ")
        )
    } else {
        format!(
            "Name the server: copal start <server>. Stopped: {}",
            names.join(", ")
        )
    };
    let index = ui.select(question, &items, &how)?;
    Ok(servers
        .into_iter()
        .nth(index)
        .expect("an index from the list"))
}

fn id(server: &Server) -> ServerId {
    ServerId {
        id: server.server_id.clone(),
    }
}

/// Every server.
pub async fn all(client: &Client) -> Result<Vec<Server>> {
    call::<ListServers>(client, &(), &(), Empty).await
}

/// The server `query` names: its name, its id, or the unique start of its id.
fn pick(servers: Vec<Server>, query: &str) -> Result<Server> {
    let query_lower = query.to_lowercase();
    let mut matches: Vec<Server> = servers
        .into_iter()
        .filter(|server| {
            server.server_id.starts_with(&query_lower)
                || server.name.to_lowercase() == query_lower
        })
        .collect();
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => bail!("there is no server named {query}. See copal list"),
        _ => bail!(
            "{query} names more than one server. Use the start of its id from copal list instead"
        ),
    }
}

pub fn state_name(state: ServerPower) -> &'static str {
    match state {
        ServerPower::Stopped => "stopped",
        ServerPower::Starting => "starting",
        ServerPower::Running => "running",
        ServerPower::Stopping => "stopping",
        ServerPower::Crashed => "crashed",
        ServerPower::Killed => "ended",
    }
}

pub fn is_up(state: ServerPower) -> bool {
    matches!(
        state,
        ServerPower::Starting | ServerPower::Running | ServerPower::Stopping
    )
}

async fn power(
    client: &Client,
    server: &Server,
    action: PowerAction,
) -> Result<()> {
    let target = id(server);
    client
        .op(|op| {
            call::<ChangePower>(
                client,
                &target,
                &(),
                Json(PowerChange { action, op }),
            )
        })
        .await
}

/// Wait, with no time limit, until the server's Java has exited.
async fn wait_offline(client: &Client, server: &Server) -> Result<Server> {
    loop {
        let now = call::<GetServer>(client, &id(server), &(), Empty).await?;
        if !is_up(now.power_state) {
            return Ok(now);
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn server(id: &str, name: &str) -> Server {
        serde_json::from_value(serde_json::json!({
            "server_id": id, "name": name, "owner_id": null, "status": "available",
            "loader": null, "loader_version": null, "mc_version": null, "upstream": null,
            "flows": {"intro": true}, "icon": null,
            "net": {"ip": null, "port": 25565, "outside": null, "game_port": 25565},
            "power_state": "stopped", "linked_instance": null, "uploaded_modpack": null,
            "shared_instance_id": null,
            "reachability": null, "players": null, "memory_mb": 2048,
            "created_at": "1970-01-01T00:00:00Z", "update_channel": "release",
            "eula_accepted": false
        }))
        .unwrap()
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

    #[test]
    fn the_plain_table_names_each_server_by_the_start_of_its_id() {
        let mut running =
            server("a1b2c3d4-0000-4000-8000-000000000001", "Survival");
        running.power_state = ServerPower::Running;
        running.mc_version = Some("1.21.1".into());
        running.players = Some(vec!["Steve".into(), "Alex".into()]);
        let servers = [
            running,
            server("ffff0000-0000-4000-8000-000000000002", "Mini"),
        ];
        assert_eq!(
            table(&servers, false),
            "Survival\trunning\t1.21.1\t25565\tSteve,Alex\ta1b2c3d4-0000-4000-8000-000000000001\n\
             Mini\tstopped\tnot set up\t25565\t\tffff0000-0000-4000-8000-000000000002"
        );
        assert_eq!(count(&servers), "2 servers, 1 running");
    }
}
