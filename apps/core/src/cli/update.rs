//! `copal update`: install a newer Core, go back to the one before, or change how it updates.

use std::time::Duration;

use color_eyre::eyre::Result;
use contracts::{
    Empty, Json,
    core::{
        ApplyUpdate, ChangeUpdateSettings, Channel, CheckForUpdate,
        CheckHealth, GetUpdate, InstallUpdate, RollBack, UpdateSettings,
        UpdateStatus,
    },
};
use serde::Serialize;

use super::{
    Client, Done, Switch,
    call::call,
    on_off,
    ui::{Mark, Ui},
};
use crate::update::{launcher, versions::describe_failure};

/// What cannot go with a change of settings, which installs nothing.
const SETTINGS_ONLY: [&str; 5] =
    ["check", "yes", "rollback", "retry", "allow_downgrade"];

#[derive(Debug, clap::Args)]
pub struct UpdateArgs {
    /// Only look for a newer release, without installing it.
    #[arg(long, conflicts_with_all = ["yes", "rollback"])]
    pub check: bool,
    /// Install without asking first.
    #[arg(short, long)]
    pub yes: bool,
    /// Change the release channel. Changes the setting and installs nothing.
    #[arg(long, conflicts_with_all = SETTINGS_ONLY)]
    pub channel: Option<Channel>,
    /// Turn automatic updates on or off. Changes the setting and installs nothing.
    #[arg(long, conflicts_with_all = SETTINGS_ONLY)]
    pub auto: Option<Switch>,
    /// Go back to the version before the last update, with the database from before it.
    /// Changes made since the update are lost.
    #[arg(long, conflicts_with_all = ["retry", "allow_downgrade"])]
    pub rollback: bool,
    /// Install the newest release even when it was tried here and rolled back.
    #[arg(long)]
    pub retry: bool,
    /// Install the channel's release even when it is older than this Core.
    #[arg(long)]
    pub allow_downgrade: bool,
}

/// `copal update`.
pub async fn run(ui: &Ui, client: &Client, args: UpdateArgs) -> Result<Done> {
    ui.intro("copal update");
    if args.channel.is_some() || args.auto.is_some() {
        let change = UpdateSettings {
            channel: args.channel,
            auto_update: args.auto.map(bool::from),
        };
        let status =
            call::<ChangeUpdateSettings>(client, &(), &(), Json(change))
                .await?;
        ui.say(Mark::Done, settings(&status));
        if ui.terminal() {
            ui.outro("Saved. Look for a newer release with: copal update");
        }
        return Ok(Done::ok(&status));
    }
    if args.rollback {
        return roll_back(ui, client, args.yes).await;
    }

    let spinner = ui.spinner("Looking for a newer release");
    let status = match call::<CheckForUpdate>(client, &(), &(), Empty).await {
        Ok(status) => status,
        Err(error) => {
            spinner.clear();
            return Err(error.wrap_err("Could not look for a newer release"));
        }
    };
    if let Some(error) = &status.check_error {
        spinner.failed(format!("Could not look for a newer release: {error}"));
        ui.outro("Core tries again by itself. copal logs says more.");
        return Ok(Done::failed(&status));
    }
    let forced = args.retry || args.allow_downgrade;
    let Some(latest) = status
        .latest
        .clone()
        .filter(|_| status.update_available || forced)
    else {
        spinner.done(format!("Core {} is up to date", status.version));
        if let Some((_, text)) = news(&status) {
            ui.say(Mark::Warning, text);
        }
        ui.outro(settings(&status));
        return Ok(Done::ok(&status));
    };
    let available =
        format!("Core {latest} is available. You have {}", status.version);
    // With nobody to ask, say so before anything is printed as if it had worked.
    if !args.check && !args.yes && !ui.terminal() {
        spinner.clear();
        color_eyre::eyre::bail!(
            "{available}. Pass --yes to install it, or --check to only look"
        );
    }
    spinner.done(available);
    if args.check {
        ui.say(Mark::Info, super::ui::dim(settings(&status)));
        ui.outro("Install it with: copal update");
        return Ok(Done::ok(&status));
    }
    let question = format!("Install Core {latest} now? Servers keep running.");
    if !args.yes && !ui.confirm_expected(&question, "--yes")? {
        ui.outro("Not installed. Install it later with: copal update");
        return Ok(Done::ok(&status));
    }
    let spinner = ui.spinner(format!("Installing Core {latest}"));
    let apply = ApplyUpdate {
        allow_downgrade: args.allow_downgrade,
        retry: args.retry,
    };
    let restarting =
        match call::<InstallUpdate>(client, &(), &(), Json(apply)).await {
            Ok(restarting) => restarting,
            Err(error) => {
                spinner.clear();
                return Err(
                    error.wrap_err(format!("Core {latest} was not installed"))
                );
            }
        };
    spinner.message(format!("Restarting into Core {}", restarting.version));
    Ok(restarted(ui, client, spinner, restarting.version, "Updated").await)
}

/// `copal update --rollback`.
async fn roll_back(ui: &Ui, client: &Client, yes: bool) -> Result<Done> {
    let status = call::<GetUpdate>(client, &(), &(), Empty).await?;
    let Some(previous) = &status.previous else {
        ui.outro(format!(
            "Core {} has no earlier version to go back to.",
            status.version
        ));
        return Ok(Done::failed(&status));
    };
    if !yes {
        if ui.terminal() {
            ui.say(
                Mark::Warning,
                format!(
                    "Going back to Core {previous} also brings back the database from before the update.\nWhat changed in Core since then is lost. Servers and their worlds are not touched."
                ),
            );
        }
        let question = format!("Go back to Core {previous}?");
        if !ui.confirm(&question, "--yes")? {
            ui.outro("Nothing was changed.");
            return Ok(Done::ok(&status));
        }
    }
    let spinner = ui.spinner(format!("Going back to Core {previous}"));
    let restarting = match call::<RollBack>(client, &(), &(), Empty).await {
        Ok(restarting) => restarting,
        Err(error) => {
            spinner.clear();
            return Err(error.wrap_err("Core did not go back"));
        }
    };
    Ok(restarted(ui, client, spinner, restarting.version, "Went back").await)
}

/// `Channel: stable. Automatic updates: on.`
fn settings(status: &UpdateStatus) -> String {
    format!(
        "Channel: {}. Automatic updates: {}.",
        format!("{:?}", status.channel).to_lowercase(),
        on_off(status.auto_update)
    )
}

/// What `copal status` says about updates, when there is something to say, and whether it is
/// a warning: a newer release, or one that was tried and rolled back.
pub fn news(status: &UpdateStatus) -> Option<(bool, String)> {
    if status.update_available
        && let Some(latest) = &status.latest
    {
        return Some((
            false,
            format!(
                "Core {latest} is available. Install it with: copal update"
            ),
        ));
    }
    let failed = status.failed.as_deref()?;
    let newer = semver::Version::parse(failed).ok()?
        > semver::Version::parse(&status.version).ok()?;
    if !newer {
        return None;
    }
    let how = status
        .failed_reason
        .map_or("did not prove healthy", describe_failure);
    Some((
        true,
        format!(
            "Core {failed} was tried and rolled back: it {how}.\nCore stays on {} until a newer release. Try {failed} again with: copal update --retry",
            status.version
        ),
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
async fn restarted(
    ui: &Ui,
    client: &Client,
    spinner: super::ui::Spinner,
    requested: String,
    done: &str,
) -> Done {
    let running = wait_for(client, &requested).await;
    let outcome = match &running {
        Some(running) if *running == requested => RestartOutcome::Running,
        Some(_) => RestartOutcome::WentBack,
        None => RestartOutcome::Unreachable,
    };
    // The launcher records how a trial failed; ask the version that took over.
    let how = match outcome {
        RestartOutcome::WentBack => call::<GetUpdate>(client, &(), &(), Empty)
            .await
            .ok()
            .filter(|status| status.failed.as_deref() == Some(&*requested))
            .and_then(|status| status.failed_reason),
        _ => None,
    }
    .map_or("did not prove healthy", describe_failure);
    match (outcome, &running) {
        (RestartOutcome::Running, _) => {
            spinner.done(format!("Core {requested} is running"));
            ui.outro(format!("{done}. Servers kept running."));
        }
        (RestartOutcome::WentBack, Some(running)) => {
            spinner.failed(format!(
                "Core {requested} {how}, so Core went back to {running}"
            ));
            ui.outro("Nothing was lost. copal logs says more.");
        }
        _ => {
            spinner.failed(format!(
                "Core did not answer after restarting into {requested}"
            ));
            ui.outro("Find out why with: copal doctor");
        }
    }
    let restarted = Restarted {
        requested,
        running,
        outcome,
    };
    if outcome == RestartOutcome::Running {
        Done::ok(&restarted)
    } else {
        Done::failed(&restarted)
    }
}

/// Wait while Core restarts, until it answers as `version` or its trial is over, and return the
/// version that answered last.
async fn wait_for(client: &Client, version: &str) -> Option<String> {
    let mut seen = None;
    let checks = launcher::HEALTH_TIMEOUT.as_secs() + 30;
    for _ in 0..checks {
        tokio::time::sleep(Duration::from_secs(1)).await;
        // What answers now, not what answered once: a version that answered and then died
        // did not take over.
        seen = call::<CheckHealth>(client, &(), &(), Empty)
            .await
            .ok()
            .map(|health| health.version);
        if seen.as_deref() == Some(version) {
            return seen;
        }
    }
    seen
}
