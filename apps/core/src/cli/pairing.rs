//! `copal pair` and `copal unpair`: joining this Core to an Amberite account, and leaving it.

use std::time::Duration;

use color_eyre::eyre::Result;
use contracts::{
    Empty,
    core::{GetPairing, PairingStatus, RequestPairingCode, Unpair},
};

use super::{
    Client, Done,
    call::call,
    status::paired_with,
    ui::{self, Mark, Ui},
};
use crate::access::pairing::instructions;

/// `copal pair`: show the code, then wait until it is approved in the App. Outside a terminal,
/// and with `no_wait`, it shows the code and leaves: `copal status` says when it is approved.
pub async fn pair(ui: &Ui, client: &Client, no_wait: bool) -> Result<Done> {
    ui.intro("copal pair");
    let mut status = call::<GetPairing>(client, &(), &(), Empty).await?;
    if status.paired {
        ui.say(Mark::Done, paired_with(&status));
        ui.outro("To pair it with another account, first unpair it with: copal unpair");
        return Ok(Done::ok(&status));
    }
    if status.code.is_none() {
        let spinner = ui.spinner("Getting a pairing code from Amberite");
        match call::<RequestPairingCode>(client, &(), &(), Empty).await {
            Ok(fresh) => {
                spinner.clear();
                status = fresh;
            }
            Err(error) => {
                spinner.clear();
                return Err(error.wrap_err("Could not get a pairing code"));
            }
        }
    }
    loop {
        if let (Some(code), Some(expires_at)) =
            (&status.code, status.expires_at)
        {
            if ui.terminal() {
                let minutes = (expires_at - chrono::Utc::now()).num_minutes();
                ui.note(
                    "Pairing code",
                    format!(
                        "{}\n\n\
                         1. Open Amberite.\n\
                         2. Go to Settings > Cores > Add Core.\n\
                         3. Enter the code. It works for {} more minutes.\n\n\
                         {}",
                        ui::bold(ui::cyan(code)),
                        minutes.max(1),
                        ui::dim(format!(
                            "Or, on the computer with Amberite, open amberite://approve/{code}"
                        )),
                    ),
                );
            } else {
                let steps = instructions(code, expires_at.timestamp());
                ui.say(Mark::Info, format!("Pairing code: {code}\n{steps}"));
            }
        }
        if no_wait || !ui.terminal() {
            ui.outro("See whether it is approved with: copal status");
            return Ok(Done::ok(&status));
        }
        let spinner = ui.spinner("Waiting for you to approve it in Amberite");
        let shown = status.code.clone();
        let waited = tokio::select! {
            waited = approved_or_replaced(client, shown) => waited,
            _ = tokio::signal::ctrl_c() => {
                spinner.clear();
                ui.outro(
                    "Stopped waiting. The code still works: see whether it is approved with copal status",
                );
                return Ok(Done::ok(&status));
            }
        };
        status = match waited {
            Ok(status) => status,
            Err(error) => {
                spinner.clear();
                return Err(error.wrap_err("Core stopped answering"));
            }
        };
        if status.paired {
            spinner.done(paired_with(&status));
            // A Core paired again may have servers already.
            let empty = super::servers::all(client)
                .await
                .is_ok_and(|servers| servers.is_empty());
            ui.outro(if empty {
                "This Core now appears in Amberite. Create its first server there."
            } else {
                "This Core now appears in Amberite."
            });
            return Ok(Done::ok(&status));
        }
        spinner.done("That code ran out. Here is a new one.");
    }
}

/// Wait until this Core is paired, or shows another code than `shown`.
async fn approved_or_replaced(
    client: &Client,
    shown: Option<String>,
) -> Result<PairingStatus> {
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status = call::<GetPairing>(client, &(), &(), Empty).await?;
        if status.paired || (status.code.is_some() && status.code != shown) {
            return Ok(status);
        }
    }
}

/// `copal unpair`.
pub async fn unpair(ui: &Ui, client: &Client, yes: bool) -> Result<Done> {
    ui.intro("copal unpair");
    let status = call::<GetPairing>(client, &(), &(), Empty).await?;
    if !status.paired {
        ui.outro("This Core is not paired. Pair it with: copal pair");
        return Ok(Done::ok(&status));
    }
    if !yes {
        if ui.terminal() {
            ui.say(
                Mark::Warning,
                format!(
                    "{}. Unpairing removes this Core from that account:\n\
                     nobody can reach it from Amberite, and what its servers share stops being shared.\n\
                     Servers, worlds and backups stay on this computer.",
                    paired_with(&status)
                ),
            );
        }
        if !ui.confirm("Unpair this Core?", "--yes")? {
            ui.outro("Nothing was changed.");
            return Ok(Done::ok(&status));
        }
    }
    let spinner = ui.spinner("Leaving the account");
    if let Err(error) = call::<Unpair>(client, &(), &(), Empty).await {
        spinner.clear();
        return Err(error.wrap_err("This Core is still paired"));
    }
    spinner.done("This Core is no longer paired");
    ui.outro("Pair it again with: copal pair");
    let status = call::<GetPairing>(client, &(), &(), Empty).await?;
    Ok(Done::ok(&status))
}
