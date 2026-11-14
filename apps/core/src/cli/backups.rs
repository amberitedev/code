//! `copal backups ...`.

use clap::Subcommand;
use color_eyre::eyre::{Result, bail};
use contracts::core::{
    BackupKind, BackupRef, BackupState, ChangeBackupSchedule, CreateBackup,
    ListBackups, NewBackup, OperationKind, OperationState, RestoreBackup,
    ScheduleUpdate, ServerBackup as Backup, ServerBackups, ServerId,
};

use super::{Client, Output, call::call, servers};

#[derive(Debug, Subcommand)]
pub enum BackupsCommand {
    /// List a server's backups, newest first.
    List { server: String },
    /// Back up a server's world now. A running server keeps running.
    Create {
        server: String,
        /// What to call it. Defaults to the date.
        #[arg(long)]
        name: Option<String>,
    },
    /// Put a backup back over the server's world. The current world is backed up first, as a
    /// "Before restore" backup, and nothing is restored if that fails.
    Restore { server: String, backup: String },
    /// Show or change how often a server is backed up and how many of its backup slots the
    /// scheduled backups take; the other slots are for manual backups.
    Schedule {
        server: String,
        /// Back up every this many hours.
        #[arg(long, conflicts_with = "off")]
        every: Option<u32>,
        /// No scheduled backups: every slot is for manual backups.
        #[arg(long, conflicts_with = "keep")]
        off: bool,
        /// How many scheduled backups to keep, each in a slot of its own.
        #[arg(long)]
        keep: Option<u32>,
    },
}

/// Why a backup's last operation failed, when it did: `create` or `restore`, and the reason Core
/// recorded.
fn failure(backup: &Backup) -> Option<String> {
    let last = backup
        .operations
        .last()
        .filter(|op| op.state == OperationState::Failed)?;
    Some(format!(
        "{} failed: {}",
        match last.kind {
            OperationKind::Create => "create",
            OperationKind::Restore => "restore",
        },
        last.error.as_deref().unwrap_or("no reason was recorded")
    ))
}

fn state(state: BackupState) -> &'static str {
    match state {
        BackupState::Queued => "queued",
        BackupState::Running => "running",
        BackupState::Failed => "failed",
        BackupState::Done => "done",
    }
}

pub async fn run(client: &Client, command: BackupsCommand) -> Result<Output> {
    match command {
        BackupsCommand::List { server } => {
            let server = servers::find(client, &server).await?;
            let mut backups = list(client, &server.id).await?.backups;
            backups.sort_by_key(|backup| std::cmp::Reverse(backup.created_at));
            let text = if backups.is_empty() {
                format!("{} has no backups yet", server.name)
            } else {
                backups
                    .iter()
                    .map(|backup| {
                        let line = format!(
                            "{}  {}  {:<11} {}{}",
                            backup.id,
                            super::local_time(backup.created_at),
                            state(backup.state),
                            backup.name,
                            if backup.kind == BackupKind::Manual {
                                ""
                            } else {
                                " (scheduled)"
                            }
                        );
                        // The reason is Core's text, which may quote file names: escaped.
                        match failure(backup) {
                            Some(failure) => {
                                format!("{line}\n    {}", super::safe(&failure))
                            }
                            None => line,
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("\n")
            };
            Ok(Output::new(text, &backups))
        }
        BackupsCommand::Create { server, name } => {
            let server = servers::find(client, &server).await?;
            let name = name.unwrap_or_else(|| {
                chrono::Local::now().format("%Y-%m-%d %H:%M").to_string()
            });
            let created = call::<CreateBackup>(
                client,
                &id(&server.id),
                &(),
                NewBackup { name },
            )
            .await?;
            Ok(Output::new(
                format!("Backing up {} ({})", server.name, created.id),
                &created,
            ))
        }
        BackupsCommand::Restore { server, backup } => {
            let server = servers::find(client, &server).await?;
            let backups = list(client, &server.id).await?.backups;
            let backup = pick(&backups, &backup)?;
            let target = BackupRef {
                id: server.id.clone(),
                backup_id: backup.id.clone(),
            };
            call::<RestoreBackup>(client, &target, &(), ()).await?;
            Ok(Output::new(
                format!("Restoring {} on {}", backup.name, server.name),
                backup,
            ))
        }
        BackupsCommand::Schedule {
            server,
            every,
            off,
            keep,
        } => {
            let server = servers::find(client, &server).await?;
            let mut current = list(client, &server.id).await?;
            let mut deleted = 0;
            if every.is_some() || off || keep.is_some() {
                let keep = if off { Some(0) } else { keep };
                if keep.is_some_and(|keep| keep > current.schedule.slots) {
                    bail!(
                        "a server has {} backup slots in all",
                        current.schedule.slots
                    );
                }
                // The owner at the keyboard: scheduled backups past the new slots go now.
                let changed = call::<ChangeBackupSchedule>(
                    client,
                    &id(&server.id),
                    &(),
                    ScheduleUpdate {
                        interval_hours: every,
                        keep,
                        delete_excess: true,
                    },
                )
                .await?;
                deleted = changed.excess.len();
                current = list(client, &server.id).await?;
            }
            let schedule = &current.schedule;
            let manual = schedule.slots.saturating_sub(schedule.keep);
            let mut text = match (schedule.keep, current.next_scheduled_at) {
                (0, _) | (_, None) => format!(
                    "{}: no scheduled backups; {manual} manual slots",
                    server.name
                ),
                (keep, Some(next)) => format!(
                    "{}: every {} hours, keeping the last {keep} ({manual} manual slots); next {}",
                    server.name,
                    schedule.interval_hours,
                    super::local_time(next)
                ),
            };
            if deleted > 0 {
                text.push_str(&format!(
                    "\ndeleted {deleted} older scheduled backup{}",
                    if deleted == 1 { "" } else { "s" }
                ));
            }
            Ok(Output::new(text, &current))
        }
    }
}

fn id(server_id: &str) -> ServerId {
    ServerId {
        id: server_id.to_owned(),
    }
}

async fn list(client: &Client, server_id: &str) -> Result<ServerBackups> {
    call::<ListBackups>(client, &id(server_id), &(), ()).await
}

/// The backup `query` names: its id, the unique start of its id, or its name.
fn pick<'a>(backups: &'a [Backup], query: &str) -> Result<&'a Backup> {
    let lower = query.to_lowercase();
    let matches: Vec<&Backup> = backups
        .iter()
        .filter(|backup| {
            backup.id.starts_with(&lower) || backup.name.to_lowercase() == lower
        })
        .collect();
    match matches.as_slice() {
        [backup] => Ok(backup),
        [] => bail!("there is no backup {query}. See copal backups list"),
        _ => bail!("{query} names more than one backup; use more of its id"),
    }
}
