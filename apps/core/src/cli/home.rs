//! `copal` typed alone: where this Core is on the way to being used, and the one next step.

use color_eyre::eyre::Result;
use contracts::{
    Empty,
    core::{CheckHealth, GetPairing},
};
use serde::Serialize;

use super::{
    Client, Done,
    call::call,
    client, servers,
    status::paired_with,
    ui::{self, Mark, Ui},
};
use crate::daemon::{install, service};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum Place {
    NotSetUp,
    NotRunning,
    NotPaired,
    Running,
}

#[derive(Serialize)]
struct Home {
    state: Place,
    /// The running version.
    version: Option<String>,
    paired: bool,
    /// The one next command. `copal setup` takes an admin.
    next: &'static str,
}

/// What `copal` alone shows: lines that are fine, the line that is not, and what to run next.
struct Found {
    place: Place,
    version: Option<String>,
    fine: Vec<String>,
    wrong: Option<String>,
    next: &'static str,
}

pub async fn run(ui: &Ui, client: &Client) -> Result<Done> {
    ui.intro("copal");
    let found = look(client).await;
    if !found.fine.is_empty() {
        ui.say(Mark::Done, found.fine.join("\n"));
    }
    if let Some(wrong) = &found.wrong {
        let mark = match found.place {
            Place::NotPaired => Mark::Warning,
            _ => Mark::Problem,
        };
        ui.say(mark, wrong);
    }
    let next = match found.next {
        "copal setup" => install::as_admin("Set it up with", found.next),
        next => format!("Next: {next}"),
    };
    ui.outro(if ui.terminal() {
        format!("{next}\n{}", ui::dim("Every command: copal --help"))
    } else {
        next
    });
    Ok(Done::ok(&Home {
        state: found.place,
        version: found.version,
        paired: found.place == Place::Running,
        next: found.next,
    }))
}

async fn look(client: &Client) -> Found {
    let Ok(health) = call::<CheckHealth>(client, &(), &(), Empty).await else {
        let (place, wrong, next) = match service::state() {
            service::State::NotRegistered => {
                (Place::NotSetUp, client::NOT_SET_UP, "copal setup")
            }
            service::State::Stopped => {
                (Place::NotRunning, client::NOT_RUNNING, "copal doctor")
            }
            service::State::Running => {
                (Place::NotRunning, client::NOT_ANSWERING, "copal doctor")
            }
        };
        return Found {
            place,
            version: None,
            fine: Vec::new(),
            wrong: Some(wrong.into()),
            next,
        };
    };
    let mut fine = vec![format!("Core {} is running", health.version)];
    let version = Some(health.version);
    match call::<GetPairing>(client, &(), &(), Empty).await {
        Ok(pairing) if pairing.paired => {
            fine.push(paired_with(&pairing));
            let (wrong, next) = match servers::all(client).await {
                Ok(servers) if servers.is_empty() => {
                    fine.push(servers::NO_SERVERS.into());
                    (None, "copal status")
                }
                Ok(servers) => {
                    fine.push(servers::count(&servers));
                    (None, "copal status")
                }
                Err(_) => (
                    Some("It does not answer as it should.".to_owned()),
                    "copal doctor",
                ),
            };
            Found {
                place: Place::Running,
                version,
                fine,
                wrong,
                next,
            }
        }
        Ok(pairing) => Found {
            place: Place::NotPaired,
            version,
            fine,
            wrong: Some(format!(
                "{} yet, so Amberite cannot use it.",
                paired_with(&pairing)
            )),
            next: "copal pair",
        },
        Err(_) => Found {
            place: Place::NotRunning,
            version,
            fine,
            wrong: Some("It does not answer as it should.".into()),
            next: "copal doctor",
        },
    }
}
