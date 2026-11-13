//! A Core's servers: listing and creating them, power, setup, `server.properties` and backups.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{CoreRole, CoreUser, ReachabilityReport};

/// A server, as someone with a role on it sees it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CoreServer {
    pub id: String,
    pub name: String,
    /// Null until the Core is paired.
    pub owner_id: Option<String>,
    /// The caller's role. Null for the owner, who has every role.
    pub role: Option<CoreRole>,
    /// Its loader or pack is installing now.
    pub installing: bool,
    /// `vanilla`, `fabric`, `quilt`, `forge`, `neoforge` or `paper`. Null until installed.
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub game_version: Option<String>,
    pub net: ServerNet,
    pub power: ServerPower,
    /// The App instance it follows.
    pub linked_instance: Option<LinkedInstance>,
    /// Whether players can join from the internet. Null while it does not run.
    pub reachability: Option<ReachabilityReport>,
    /// Who is online. Null while it does not run.
    pub players: Option<Vec<String>>,
    pub memory_mb: u32,
    /// Unix seconds.
    pub created_at: i64,
    pub update_channel: UpdateChannel,
    /// The owner accepted the Minecraft EULA for every server on this Core. None starts before.
    pub eula_accepted: bool,
}

/// Where players join a server.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerNet {
    /// With `port`, where players join from the internet: the router's address when it forwarded
    /// another port than the game port, otherwise the Core's.
    pub ip: Option<String>,
    pub port: u16,
    /// Where the server is on the internet, while it runs and is forwarded or public.
    pub outside: Option<JoinAddress>,
    /// The port the server listens on.
    pub game_port: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct JoinAddress {
    pub ip: String,
    pub port: u16,
}

/// Where a server is in its run, or how its last run ended: `offline` (stopped or never run),
/// `crashed` or `killed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ServerPower {
    Offline,
    Starting,
    Running,
    Stopping,
    Crashed,
    Killed,
}

/// The App instance a server follows.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LinkedInstance {
    /// The App's id for the instance.
    pub id: String,
    pub name: String,
    pub state: LinkedPackState,
    pub error: Option<String>,
    pub shared_instance_id: Option<String>,
    /// The version of the instance's share the server runs.
    pub version: Option<i64>,
}

/// Where a linked server is with the newest version pushed to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum LinkedPackState {
    UpToDate,
    Downloading,
    /// Applies when the server next stops.
    Pending,
    Applying,
    Failed,
}

/// Which Modrinth versions a server's content updates to.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum UpdateChannel {
    Release,
    Beta,
    Alpha,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewServer {
    /// 1 to 64 characters.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerUpdate {
    /// 1 to 64 characters, trimmed.
    pub name: Option<String>,
    pub update_channel: Option<UpdateChannel>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum PowerAction {
    Start,
    Stop,
    Restart,
    Kill,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PowerChange {
    pub action: PowerAction,
}

/// A loader to install on a stopped server. Packs install through the content routes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct LoaderInstall {
    /// As [`CoreServer::loader`] names it.
    pub loader: String,
    /// Null for the newest.
    pub loader_version: Option<String>,
    pub game_version: String,
}

/// How a server starts, and what it shares.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerSetup {
    /// The Java major it runs. Null until installed.
    pub java_version: Option<u32>,
    /// The command line it runs, as people read it. Null until installed.
    pub invocation: Option<String>,
    /// The share it reads: its instance's when it follows one, else its own once it is shared.
    pub shared_instance_id: Option<String>,
    /// It has changes its own share's latest version does not. Always false while it follows an
    /// instance.
    pub shared_instance_needs_update: bool,
    /// The Core machine's CPU threads.
    pub cpus: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct JavaMajor {
    /// A Java major the Core installs (8, 11, 17, 21 or 25), or null for the one the Minecraft
    /// version needs.
    pub major: Option<u32>,
}

/// Every key of a server's `server.properties`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerProperties {
    pub properties: BTreeMap<String, String>,
}

/// Used from the server's next start.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct PropertiesUpdate {
    /// Null removes the key.
    pub properties: BTreeMap<String, Option<String>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerBackups {
    pub backups: Vec<ServerBackup>,
    pub schedule: BackupSchedule,
    /// Unix seconds. Null while scheduled backups are off.
    pub next_scheduled_at: Option<i64>,
}

/// A server has `slots` backup slots: `keep` for scheduled backups, the rest for manual ones.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BackupSchedule {
    pub interval_hours: u32,
    pub keep: u32,
    pub slots: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ServerBackup {
    pub id: String,
    pub name: String,
    /// Unix seconds.
    pub created_at: i64,
    pub state: BackupState,
    pub kind: BackupKind,
    /// Unix seconds. Set on scheduled backups that hold a slot: when later scheduled backups push
    /// it out.
    pub rotates_at: Option<i64>,
    pub operations: Vec<BackupOperation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BackupState {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum BackupKind {
    Manual,
    Scheduled,
    /// Made before a pack update or a restore.
    PreUpdate,
}

/// Making a backup or restoring it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BackupOperation {
    pub id: i64,
    pub kind: OperationKind,
    pub state: OperationState,
    /// Unix seconds, as are the other times.
    pub queued_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub error: Option<String>,
    /// Someone dismissed it once it ended.
    pub acked: bool,
    /// Who started it. Null for the schedule.
    pub user: Option<CoreUser>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Create,
    Restore,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum OperationState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct NewBackup {
    /// 1 to 64 characters.
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct CreatedBackup {
    pub id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BackupUpdate {
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BackupIds {
    pub ids: Vec<String>,
}

/// What is given changes; the rest stays.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScheduleUpdate {
    /// Hourly at most, every 744 hours at least.
    pub interval_hours: Option<u32>,
    /// 0 turns scheduled backups off.
    pub keep: Option<u32>,
    /// Keeping fewer than there are deletes the oldest scheduled backups. Without this the change
    /// is only described; with it, it is the owner's dangerous action.
    pub delete_excess: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct ScheduleChange {
    pub applied: bool,
    /// The scheduled backups the change deletes, or deleted once applied.
    pub excess: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct BackupRef {
    /// The server.
    pub id: String,
    pub backup_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub struct OperationRef {
    /// The server.
    pub id: String,
    pub operation_id: i64,
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        CoreServer::decl(&cfg),
        ServerNet::decl(&cfg),
        JoinAddress::decl(&cfg),
        ServerPower::decl(&cfg),
        LinkedInstance::decl(&cfg),
        LinkedPackState::decl(&cfg),
        UpdateChannel::decl(&cfg),
        NewServer::decl(&cfg),
        ServerUpdate::decl(&cfg),
        PowerAction::decl(&cfg),
        PowerChange::decl(&cfg),
        LoaderInstall::decl(&cfg),
        ServerSetup::decl(&cfg),
        JavaMajor::decl(&cfg),
        ServerProperties::decl(&cfg),
        PropertiesUpdate::decl(&cfg),
        ServerBackups::decl(&cfg),
        BackupSchedule::decl(&cfg),
        ServerBackup::decl(&cfg),
        BackupState::decl(&cfg),
        BackupKind::decl(&cfg),
        BackupOperation::decl(&cfg),
        OperationKind::decl(&cfg),
        OperationState::decl(&cfg),
        NewBackup::decl(&cfg),
        CreatedBackup::decl(&cfg),
        BackupUpdate::decl(&cfg),
        BackupIds::decl(&cfg),
        ScheduleUpdate::decl(&cfg),
        ScheduleChange::decl(&cfg),
        BackupRef::decl(&cfg),
        OperationRef::decl(&cfg),
    ]
}
