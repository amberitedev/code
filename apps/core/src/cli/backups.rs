//! `copal backups ...`, on the backup routes of step 5 (`api::backups`). A server's one world has
//! the server's id, so its backups are under `/v1/servers/{id}/worlds/{id}/backups-queue`.
//!
//! The answer structs in `api::backups` only serialize, so the CLI reads the fields it shows
//! with its own, matching by name.

use axum::http::Method;
use clap::Subcommand;
use color_eyre::eyre::{Result, bail};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{Client, Output, servers};

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
    /// Show or change how often a server is backed up and how many backups it keeps.
    Schedule {
        server: String,
        /// Back up every this many hours.
        #[arg(long, conflicts_with = "off")]
        every: Option<u32>,
        /// No scheduled backups.
        #[arg(long)]
        off: bool,
        /// How many scheduled backups to keep.
        #[arg(long)]
        keep: Option<u32>,
    },
}

/// One backup in `api::backups::BackupsQueueResponse`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Backup {
    pub id: Uuid,
    pub name: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub status: String,
    pub automated: bool,
    /// Its creates and restores, oldest first.
    #[serde(default)]
    pub history: Vec<Operation>,
}

/// One entry of a backup's `history` in `api::backups::BackupQueueBackup`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Operation {
    pub operation_type: String,
    pub state: String,
    pub error: Option<String>,
}

impl Backup {
    /// Why its last operation failed, when it did: `create` or `restore`, and the reason Core
    /// recorded.
    pub fn failure(&self) -> Option<String> {
        let last = self.history.last().filter(|op| op.state == "failed")?;
        Some(format!(
            "{} failed: {}",
            last.operation_type,
            last.error.as_deref().unwrap_or("no reason was recorded")
        ))
    }
}

#[derive(Debug, Deserialize)]
struct Backups {
    backups: Vec<Backup>,
}

/// `api::backups::BackupRequest`.
#[derive(Debug, Serialize)]
struct BackupRequest {
    name: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Created {
    id: Uuid,
}

/// A server's backup schedule. Step 5 has no type for it yet; this is the shape the CLI sends to
/// `GET|PUT /servers/{id}/backups/schedule`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BackupSchedule {
    /// Null: no scheduled backups.
    pub every_hours: Option<u32>,
    pub keep: u32,
}

pub async fn run(client: &Client, command: BackupsCommand) -> Result<Output> {
    match command {
        BackupsCommand::List { server } => {
            let server = servers::find(client, &server).await?;
            let mut backups = list(client, &server.server_id).await?;
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
                            backup
                                .created_at
                                .with_timezone(&chrono::Local)
                                .format("%Y-%m-%d %H:%M"),
                            backup.status,
                            backup.name,
                            if backup.automated { " (scheduled)" } else { "" }
                        );
                        // The reason is Core's text, which may quote file names: escaped.
                        match backup.failure() {
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
            let created: Created = client
                .post(&queue(&server.server_id), &BackupRequest { name })
                .await?;
            Ok(Output::new(
                format!("Backing up {} ({})", server.name, created.id),
                &created,
            ))
        }
        BackupsCommand::Restore { server, backup } => {
            let server = servers::find(client, &server).await?;
            let backups = list(client, &server.server_id).await?;
            let backup = pick(&backups, &backup)?;
            client
                .post::<()>(
                    &format!(
                        "{}/{}/restore",
                        queue(&server.server_id),
                        backup.id
                    ),
                    &BackupRequest {
                        name: format!("Before restoring {}", backup.name),
                    },
                )
                .await?;
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
            let path =
                format!("/servers/{}/backups/schedule", server.server_id);
            let mut schedule: BackupSchedule = client.get(&path).await?;
            if every.is_some() || off || keep.is_some() {
                if every.is_some() || off {
                    schedule.every_hours = every;
                }
                if let Some(keep) = keep {
                    schedule.keep = keep;
                }
                schedule =
                    client.call(Method::PUT, &path, Some(&schedule)).await?;
            }
            let text = match schedule.every_hours {
                Some(hours) => format!(
                    "{}: every {hours} hours, keeping the last {}",
                    server.name, schedule.keep
                ),
                None => format!("{}: no scheduled backups", server.name),
            };
            Ok(Output::new(text, &schedule))
        }
    }
}

fn queue(server_id: &Uuid) -> String {
    format!("/v1/servers/{server_id}/worlds/{server_id}/backups-queue")
}

async fn list(client: &Client, server_id: &Uuid) -> Result<Vec<Backup>> {
    let backups: Backups = client.get(&queue(server_id)).await?;
    Ok(backups.backups)
}

/// The backup `query` names: its id, the unique start of its id, or its name.
fn pick<'a>(backups: &'a [Backup], query: &str) -> Result<&'a Backup> {
    let lower = query.to_lowercase();
    let matches: Vec<&Backup> = backups
        .iter()
        .filter(|backup| {
            backup.id.to_string().starts_with(&lower)
                || backup.name.to_lowercase() == lower
        })
        .collect();
    match matches.as_slice() {
        [backup] => Ok(backup),
        [] => bail!("there is no backup {query}. See copal backups list"),
        _ => bail!("{query} names more than one backup; use more of its id"),
    }
}
