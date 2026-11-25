//! `copal access ...`: the devices signed in to this Core.

use clap::Subcommand;
use color_eyre::eyre::{Result, bail};
use contracts::{
    Empty,
    core::{CoreSession, ListSessions, RevokeSession, SessionId},
};

use super::{Client, Output, call::call, local_time};

#[derive(Debug, Subcommand)]
pub enum AccessCommand {
    /// Every signed-in device.
    Sessions,
    /// End a session. The device has to sign in again.
    Revoke { session: String },
}

pub async fn run(client: &Client, command: AccessCommand) -> Result<Output> {
    match command {
        AccessCommand::Sessions => {
            let sessions =
                call::<ListSessions>(client, &(), &(), Empty).await?;
            let text = if sessions.is_empty() {
                "No sessions".to_owned()
            } else {
                sessions
                    .iter()
                    .map(|session| {
                        format!(
                            "{}  {:<16} {:<24} last used {}",
                            session.id,
                            session.user.name,
                            session.device_name,
                            local_time(session.last_used_at)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(Output::new(text, &sessions))
        }
        AccessCommand::Revoke { session } => {
            let sessions =
                call::<ListSessions>(client, &(), &(), Empty).await?;
            let matches: Vec<&CoreSession> = sessions
                .iter()
                .filter(|candidate| candidate.id.starts_with(&session))
                .collect();
            let [session] = matches.as_slice() else {
                bail!(
                    "{session} names {} sessions. See copal access sessions",
                    if matches.is_empty() { "no" } else { "several" }
                );
            };
            let id = SessionId {
                id: session.id.clone(),
            };
            call::<RevokeSession>(client, &id, &(), Empty).await?;
            Ok(Output::new(
                format!(
                    "Ended the session of {} on {}",
                    session.user.name, session.device_name
                ),
                session,
            ))
        }
    }
}
