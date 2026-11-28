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
    ui.begin();
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
                        ui::bold(ui::paint(code, ui::Tone::Accent)),
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
            waited = approved_or_replaced(client, shown, status.expires_at) => waited,
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
        if status.code.is_none() {
            spinner.failed(
                "That code ran out, and Core could not get a new one from Amberite",
            );
            ui.outro("Try again with: copal pair");
            return Ok(Done::failed(&status));
        }
        spinner.done("That code ran out. Here is a new one.");
    }
}

/// Wait until this Core is paired, or shows another code than `shown`, or `shown` ran out
/// with no code after it, which is a Core that cannot reach Amberite for another.
async fn approved_or_replaced(
    client: &Client,
    shown: Option<String>,
    expires_at: Option<chrono::DateTime<chrono::Utc>>,
) -> Result<PairingStatus> {
    // A new code takes Core a moment to get: only a longer gap is a code that will not come.
    let gone = expires_at.map(|at| at + chrono::Duration::seconds(30));
    loop {
        tokio::time::sleep(Duration::from_secs(1)).await;
        let status = call::<GetPairing>(client, &(), &(), Empty).await?;
        if status.paired || (status.code.is_some() && status.code != shown) {
            return Ok(status);
        }
        if status.code.is_none()
            && gone.is_some_and(|gone| chrono::Utc::now() > gone)
        {
            return Ok(status);
        }
    }
}

/// `copal unpair`.
pub async fn unpair(ui: &Ui, client: &Client, yes: bool) -> Result<Done> {
    ui.begin();
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
        // Core said no, or said nothing: only the first is a Core that is still paired.
        return Err(if error.is::<super::client::Refused>() {
            error.wrap_err("This Core is still paired")
        } else {
            error.wrap_err(
                "Unpairing could not be confirmed. See whether it worked with: copal status",
            )
        });
    }
    spinner.done("This Core is no longer paired");
    ui.outro("Pair it again with: copal pair");
    // It worked whatever Core says next, so what it says next cannot fail it.
    let status = call::<GetPairing>(client, &(), &(), Empty).await.unwrap_or(
        PairingStatus {
            paired: false,
            owner: None,
            code: None,
            expires_at: None,
        },
    );
    Ok(Done::ok(&status))
}
