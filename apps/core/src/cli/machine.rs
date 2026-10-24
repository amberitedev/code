//! Core itself: `status`, `logs`, `update`, and `eula`.

use std::time::Duration;

use axum::http::Method;
use clap::Subcommand;
use color_eyre::eyre::Result;
use serde::{Deserialize, Serialize};

use super::{Client, Output, Switch, local_time, on_off, servers};
use crate::{
    access::pairing,
    api::{
        eula::Eula,
        insight::{Machine, Reach},
        logs::CoreLogs,
        pairing::PairingStatus,
        update::{ApplyUpdate, Restarting, UpdateSettings, UpdateStatus},
    },
    update::{launcher, release::Channel, versions::FailedTrial},
};

#[derive(Debug, Subcommand)]
pub enum UpdateCommand {
    /// Look for a newer release on the channel.
    Check,
    /// Install the newest release on the channel and restart Core into it. Servers keep
    /// running.
    Apply {
        /// Install the channel's release even when it is older than this Core.
        #[arg(long)]
        allow_downgrade: bool,
        /// Install the channel's release even when it was tried here and rolled back.
        #[arg(long)]
        retry: bool,
    },
    /// Show or change the release channel.
    Channel { channel: Option<Channel> },
    /// Show or change whether updates install by themselves.
    Auto { switch: Option<Switch> },
    /// Go back to the version before the last update, with the database from before it.
    /// Changes made since the update are lost.
    Rollback,
}

/// What `/health` answers, as the CLI reads it.
#[derive(Debug, Deserialize, Serialize)]
pub struct Health {
    pub version: String,
    pub core_id: String,
}

/// `copal status`.
pub async fn status(client: &Client) -> Result<Output> {
    #[derive(Serialize)]
    struct Status {
        health: Health,
        update: UpdateStatus,
        machine: Machine,
        pairing: PairingStatus,
        servers: Vec<crate::api::servers::Server>,
    }
    let health: Health = client.get("/health").await?;
    let update: UpdateStatus = client.get("/update").await?;
    let machine: Machine = client.get("/machine").await?;
    let pairing: PairingStatus = client.get("/pairing").await?;
    let servers = servers::list(client).await?;
    let mut lines = vec![format!(
        "Core {} is running (id {})",
        health.version, health.core_id
    )];
    if !pairing.paired {
        lines.push(pairing_text(&pairing));
    }
    if update.update_available
        && let Some(latest) = &update.latest
    {
        lines.push(format!("Copal {latest} is available: copal update apply"));
    }
    lines.extend(failed_update(&update));
    if let Some(memory_mb) = machine.memory_mb {
        lines.push(format!(
            "Memory: the servers may use {} of this machine's {memory_mb} MB",
            machine.assigned_mb
        ));
    }
    lines.extend(machine.warning.clone());
    if let Some(api) = &machine.api_reachability
        && api.state == Reach::Unreachable
    {
        lines.push(format!(
            "The App cannot reach Core from the internet: {}",
            api.title
        ));
        lines.extend(api.next_step.iter().map(|step| format!("Next: {step}")));
    }
    lines.push(servers::table(&servers));
    Ok(Output::new(
        lines.join("\n"),
        &Status {
            health,
            update,
            machine,
            pairing,
            servers,
        },
    ))
}

/// `copal pair`.
pub async fn pair(client: &Client) -> Result<Output> {
    let status: PairingStatus = client.post("/pairing/code", &()).await?;
    Ok(Output::new(pairing_text(&status), &status))
}

/// What `status` and `pair` say about pairing.
fn pairing_text(status: &PairingStatus) -> String {
    match (&status.code, status.expires_at) {
        _ if status.paired => "This Core is paired".into(),
        (Some(code), Some(expires_at)) => format!(
            "This Core is not paired yet.\n{}\nFor a new code: copal pair",
            pairing::instructions(code, expires_at)
        ),
        _ => "This Core is not paired yet and is still getting a code from Amberite. copal logs says why.".into(),
    }
}

/// `copal logs`.
pub async fn logs(client: &Client, lines: usize) -> Result<Output> {
    let logs: CoreLogs = client.get(&format!("/logs?lines={lines}")).await?;
    Ok(Output::new(super::safe(&logs.lines.join("\n")), &logs))
}

/// `copal eula`.
pub async fn eula(client: &Client, accept: bool) -> Result<Output> {
    let eula: Eula = if accept {
        client.post("/eula", &()).await?
    } else {
        client.get("/eula").await?
    };
    let text = match eula.accepted_at {
        Some(at) => format!(
            "The Minecraft EULA ({}) was accepted on {}",
            eula.url,
            local_time(at)
        ),
        None => format!(
            "Servers start once you accept the Minecraft EULA ({}): copal eula --accept",
            eula.url
        ),
    };
    Ok(Output::new(text, &eula))
}

/// `copal update ...`.
pub async fn update(client: &Client, command: UpdateCommand) -> Result<Output> {
    match command {
        UpdateCommand::Check => {
            let status: UpdateStatus =
                client.post("/update/check", &()).await?;
            Ok(Output::new(describe(&status), &status))
        }
        UpdateCommand::Apply {
            allow_downgrade,
            retry,
        } => {
            let restarting: Restarting = client
                .post(
                    "/update/apply",
                    &ApplyUpdate {
                        allow_downgrade,
                        retry,
                    },
                )
                .await?;
            Ok(restarted(client, restarting.version).await)
        }
        UpdateCommand::Channel { channel } => {
            let status = settings(
                client,
                UpdateSettings {
                    channel,
                    ..UpdateSettings::default()
                },
            )
            .await?;
            let text = format!("Channel: {:?}", status.channel).to_lowercase();
            Ok(Output::new(text, &status))
        }
        UpdateCommand::Auto { switch } => {
            let status = settings(
                client,
                UpdateSettings {
                    auto_update: switch.map(bool::from),
                    ..UpdateSettings::default()
                },
            )
            .await?;
            let text =
                format!("Automatic updates: {}", on_off(status.auto_update));
            Ok(Output::new(text, &status))
        }
        UpdateCommand::Rollback => {
            let restarting: Restarting =
                client.post("/update/rollback", &()).await?;
            Ok(restarted(client, restarting.version).await)
        }
    }
}

/// Show the settings, or change the ones given.
async fn settings(
    client: &Client,
    change: UpdateSettings,
) -> Result<UpdateStatus> {
    if change.channel.is_none() && change.auto_update.is_none() {
        return client.get("/update").await;
    }
    client.call(Method::PATCH, "/update", Some(&change)).await
}

fn describe(status: &UpdateStatus) -> String {
    match (&status.latest, status.update_available) {
        (Some(latest), true) => format!(
            "Copal {latest} is available (this is {}): copal update apply",
            status.version
        ),
        _ => failed_update(status).unwrap_or_else(|| {
            format!("Copal {} is up to date", status.version)
        }),
    }
}

/// The update that was tried and rolled back, while it is still newer than the running
/// version: what happened, and how to try it again.
fn failed_update(status: &UpdateStatus) -> Option<String> {
    let failed = status.failed.as_deref()?;
    let newer = semver::Version::parse(failed).ok()?
        > semver::Version::parse(&status.version).ok()?;
    if !newer {
        return None;
    }
    let how = status
        .failed_reason
        .map_or("did not prove healthy", FailedTrial::describe);
    Some(format!(
        "Copal {failed} was tried and rolled back: it {how}. Core stays on {} until a newer release; to try {failed} again: copal update apply --retry",
        status.version
    ))
}

/// How a restart into another version went, as the CLI saw it.
#[derive(Debug, Serialize)]
pub struct Restarted {
    /// The version Core restarted into.
    pub requested: String,
    /// The version that answered last, `None` when none did.
    pub running: Option<String>,
    pub outcome: RestartOutcome,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RestartOutcome {
    /// `requested` answers.
    Running,
    /// Another version answers: the launcher went back to it.
    WentBack,
    /// No version answered in time.
    Unreachable,
}

/// Wait while Core restarts into `requested`, and say how it went. Anything but `requested`
/// answering in the end is a failure.
async fn restarted(client: &Client, requested: String) -> Output {
    let running = wait_for(client, &requested).await;
    let outcome = match &running {
        Some(running) if *running == requested => RestartOutcome::Running,
        Some(_) => RestartOutcome::WentBack,
        None => RestartOutcome::Unreachable,
    };
    // The launcher records how a trial failed; ask the version that took over.
    let how = match outcome {
        RestartOutcome::WentBack => client
            .get::<UpdateStatus>("/update")
            .await
            .ok()
            .filter(|status| status.failed.as_deref() == Some(&*requested))
            .and_then(|status| status.failed_reason),
        _ => None,
    }
    .map_or("did not prove healthy", FailedTrial::describe);
    let text = match (outcome, &running) {
        (RestartOutcome::Running, _) => format!("Core {requested} is running"),
        (RestartOutcome::WentBack, Some(running)) => format!(
            "Copal {requested} {how}, so the launcher went back to {running}. See copal logs"
        ),
        _ => format!(
            "Core did not answer after restarting into {requested}. See the service's log"
        ),
    };
    let restarted = Restarted {
        requested,
        running,
        outcome,
    };
    if outcome == RestartOutcome::Running {
        Output::new(text, &restarted)
    } else {
        Output::failed(text, &restarted)
    }
}

/// Wait while Core restarts, until it answers as `version` or its trial is over, and return the
/// version that answered last.
async fn wait_for(client: &Client, version: &str) -> Option<String> {
    let mut seen = None;
    let checks = launcher::HEALTH_TIMEOUT.as_secs() + 30;
    for _ in 0..checks {
        tokio::time::sleep(Duration::from_secs(1)).await;
        if let Ok(health) = client.get::<Health>("/health").await {
            if health.version == version {
                return Some(health.version);
            }
            seen = Some(health.version);
        }
    }
    seen
}
