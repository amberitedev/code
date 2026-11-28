//! `copal status` and `copal logs`: Core as it is now.

use color_eyre::eyre::Result;
use contracts::{
    Empty,
    core::{
        CheckHealth, GetLogs, GetMachine, GetPairing, GetUpdate, Health,
        LogsQuery, Machine, PairingStatus, UpdateStatus,
    },
};
use serde::Serialize;

use super::{
    Client, Done,
    call::call,
    client::Unreachable,
    servers,
    ui::{self, Mark, Ui},
    update,
};
use crate::daemon::{config, paths::Paths};

/// `copal status`.
pub async fn run(ui: &Ui, client: &Client) -> Result<Done> {
    #[derive(Serialize)]
    struct Status {
        health: Health,
        update: UpdateStatus,
        machine: Machine,
        pairing: PairingStatus,
        servers: Vec<contracts::core::Server>,
    }
    ui.begin();
    let health = call::<CheckHealth>(client, &(), &(), Empty).await?;
    let update = call::<GetUpdate>(client, &(), &(), Empty).await?;
    let machine = call::<GetMachine>(client, &(), &(), Empty).await?;
    let pairing = call::<GetPairing>(client, &(), &(), Empty).await?;
    let servers = servers::all(client).await?;

    let mut core = vec![format!("Core {} is running", health.version)];
    if pairing.paired {
        core.push(paired_with(&pairing));
    }
    if let Some(memory) = memory(&machine) {
        core.push(memory);
    }
    ui.say(Mark::Done, core.join("\n"));
    if !pairing.paired {
        ui.say(
            Mark::Warning,
            "Not paired with an Amberite account yet, so Amberite cannot use it.\nPair it with: copal pair",
        );
    }
    if let Some(warning) = &machine.warning {
        ui.say(Mark::Warning, warning);
    }
    match update::news(&update) {
        Some((true, text)) => ui.say(Mark::Warning, text),
        Some((false, text)) => ui.say(Mark::Info, text),
        None => {}
    }
    if servers.is_empty() {
        ui.say(Mark::Info, servers::NO_SERVERS);
    } else {
        ui.say(
            Mark::Done,
            format!(
                "{}\n{}",
                servers::count(&servers),
                // Drawn for a reader either way: `copal list` is the one for scripts.
                servers::table(&servers, true)
            ),
        );
    }
    if ui.terminal() {
        // A crashed server is the App's to explain; everything else is the doctor's.
        let crashed = servers.iter().find(|server| {
            server.power_state == contracts::core::ServerPower::Crashed
        });
        ui.outro(match crashed {
            Some(server) => {
                let name = super::safe(&server.name);
                // Named only when `copal start` takes the name as it is: one plain word
                // that picks this server alone. The list to pick from, otherwise.
                let plain = name.starts_with(char::is_alphanumeric)
                    && name
                        .chars()
                        .all(|c| c.is_alphanumeric() || c == '-' || c == '_');
                let lower = name.to_lowercase();
                let picked = servers
                    .iter()
                    .filter(|other| {
                        other.server_id.starts_with(&lower)
                            || other.name.to_lowercase() == lower
                    })
                    .count();
                let named = if plain && picked == 1 {
                    format!(" {name}")
                } else {
                    String::new()
                };
                format!(
                    "{name} crashed. See why in Amberite, or start it again with: copal start{named}"
                )
            }
            None => "Something not working? Find out with: copal doctor"
                .to_owned(),
        });
    }
    Ok(Done::ok(&Status {
        health,
        update,
        machine,
        pairing,
        servers,
    }))
}

/// `Paired with ilai`, or that it is not paired.
pub fn paired_with(pairing: &PairingStatus) -> String {
    match (pairing.paired, &pairing.owner) {
        (true, Some(owner)) => format!("Paired with {}", super::safe(owner)),
        (true, None) => "Paired with an Amberite account".into(),
        (false, _) => "Not paired with an Amberite account".into(),
    }
}

/// How much of the machine's memory the servers may use together.
fn memory(machine: &Machine) -> Option<String> {
    let gb = |mb: f64| format!("{:.1} GB", mb / 1024.0);
    let total = machine.memory_mb.filter(|_| machine.assigned_mb > 0)?;
    Some(format!(
        "Servers are given {} of this computer's {} of memory",
        gb(machine.assigned_mb as f64),
        gb(total as f64)
    ))
}

/// `copal logs`: the end of Core's log, from Core. When Core does not answer, which is when
/// its log matters most, from the file itself, which an admin may read.
pub async fn logs(ui: &Ui, client: &Client, lines: usize) -> Result<Done> {
    let query = LogsQuery { lines: Some(lines) };
    let logs = match call::<GetLogs>(client, &(), &query, Empty).await {
        Ok(logs) => logs,
        // Core is down, or refuses this user (root, under sudo): the file itself, then. An
        // answer Core did give, whatever it was, is not replaced by an old log.
        Err(error)
            if error.is::<Unreachable>()
                || error.is::<super::client::Closed>() =>
        {
            let folder = Paths::new(config::data_dir()).logs();
            let lines = crate::api::logs::tail(folder, lines.clamp(1, 1000));
            if lines.is_empty() && !error.is::<Unreachable>() {
                return Err(error);
            }
            if lines.is_empty() {
                ui.begin();
                let mut stuck = super::client::why_unreachable();
                // Where Core was set up there is a log, which only an admin may read: someone
                // who is not one is told how. An admin who got nothing has no log to read.
                if crate::daemon::service::state()
                    != crate::daemon::service::State::NotRegistered
                    && !crate::daemon::install::is_admin()
                {
                    stuck.next = crate::daemon::install::as_admin(
                        "Read its log with",
                        "copal logs",
                    );
                }
                return Err(stuck.into());
            }
            contracts::core::CoreLogs { lines }
        }
        Err(error) => return Err(error),
    };
    // No frame: a log is read, searched and copied as it is.
    for line in &logs.lines {
        let line = super::safe(line);
        if ui.terminal() {
            println!("{}", ui::folded(&coloured(&line)));
        } else {
            ui.say(Mark::Info, line);
        }
    }
    Ok(Done::ok(&logs))
}

/// A log line for a terminal: its time as this computer's own, and its level standing out.
fn coloured(line: &str) -> String {
    let line = match line.split_once(' ') {
        Some((time, rest)) => {
            match time.parse::<chrono::DateTime<chrono::Utc>>() {
                Ok(time) => format!(
                    "{} {rest}",
                    time.with_timezone(&chrono::Local).format("%b %d %H:%M:%S")
                ),
                Err(_) => line.to_owned(),
            }
        }
        None => line.to_owned(),
    };
    if let Some((before, after)) = line.split_once(" ERROR ") {
        format!("{}{}{after}", ui::dim(before), ui::red(" ERROR "))
    } else if let Some((before, after)) = line.split_once(" WARN ") {
        format!("{}{}{after}", ui::dim(before), ui::yellow(" WARN "))
    } else if let Some((before, after)) = line.split_once(" INFO ") {
        format!("{}{after}", ui::dim(format!("{before} INFO ")))
    } else {
        line
    }
}
