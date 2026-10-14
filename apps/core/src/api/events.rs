//! Live updates: the console socket `GET /servers/{id}/ws`, the `GET /v1/sync` event stream, and
//! `POST /servers/{id}/v1/logs/clear` (no body, no response). Mirrors `Archon.Websocket.v0` and
//! `Archon.Sync.v1` in `packages/api-client/src/modules/archon/types.ts`.

#![allow(dead_code)] // Wired in step 2: console and event routes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::content::{
    Addon, AddonStatus, AddonVersion, Addons, ContentError, ContentOwner,
    Environment, Installing,
};

/// A message the App sends on the console socket. Mirrors Archon.Websocket.v0.WSOutgoingMessage.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum WsClientMessage {
    /// Mirrors Archon.Websocket.v0.WSAuthMessage.
    Auth { jwt: String },
    /// Mirrors Archon.Websocket.v0.WSCommandMessage.
    Command { cmd: String },
}

/// A message Core sends on the console socket. Mirrors Archon.Websocket.v0.WSEvent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
pub enum WsEvent {
    /// Mirrors Archon.Websocket.v0.WSBackupProgressEvent.
    BackupProgress {
        id: Uuid,
        task: BackupTask,
        state: BackupState,
        progress: f64,
        start_time: Option<i64>,
        finish_time: Option<i64>,
    },
    /// Mirrors Archon.Websocket.v0.WSLogEvent.
    Log { stream: LogStream, message: String },
    /// Mirrors Archon.Websocket.v0.WSLog4jEvent.
    Log4j {
        #[serde(skip_serializing_if = "Option::is_none")]
        logger_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        level: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        thread_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        timestamp_millis: Option<i64>,
        #[serde(skip_serializing_if = "Option::is_none")]
        message: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        throwable: Option<String>,
    },
    /// Mirrors Archon.Websocket.v0.WSStatsEvent.
    Stats {
        cpu_percent: f64,
        memory_percent: Option<f64>,
        ram_usage_bytes: u64,
        ram_total_bytes: u64,
        storage_usage_bytes: u64,
        storage_total_bytes: u64,
        net_tx_bytes: u64,
        net_rx_bytes: u64,
    },
    /// Mirrors Archon.Websocket.v0.WSPowerStateEvent.
    PowerState {
        state: PowerState,
        #[serde(skip_serializing_if = "Option::is_none")]
        oom_killed: Option<bool>,
        #[serde(skip_serializing_if = "Option::is_none")]
        exit_code: Option<i32>,
    },
    /// Mirrors Archon.Websocket.v0.WSStateEvent.
    State {
        debug: String,
        power_variant: FlattenedPowerState,
        exit_code: Option<i32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        was_oom: Option<bool>,
        target: Option<PowerTarget>,
        uptime: u64,
        progress: Option<SyncContentProgress>,
        content_error: Option<SyncContentError>,
    },
    /// Mirrors Archon.Websocket.v0.WSAuthExpiringEvent.
    AuthExpiring,
    /// Mirrors Archon.Websocket.v0.WSAuthIncorrectEvent.
    AuthIncorrect,
    /// Mirrors Archon.Websocket.v0.WSAuthOkEvent.
    AuthOk,
    /// Mirrors Archon.Websocket.v0.WSInstallationResultEvent.
    InstallationResult(InstallationResult),
    /// Mirrors Archon.Websocket.v0.WSUptimeEvent.
    Uptime { uptime: u64 },
    /// Mirrors Archon.Websocket.v0.WSNewModEvent.
    NewMod {
        project_id: String,
        version_id: String,
    },
    /// Mirrors Archon.Websocket.v0.WSInstallProgressEvent.
    InstallProgress { items: Vec<InstallProgressItem> },
    /// Mirrors Archon.Websocket.v0.WSFilesystemOpsEvent.
    FilesystemOps { all: Vec<FilesystemOperation> },
}

/// Mirrors Archon.Websocket.v0.BackupTask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupTask {
    File,
    Create,
    Restore,
}

/// Mirrors Archon.Websocket.v0.BackupState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupState {
    Pending,
    Ongoing,
    Done,
    Failed,
    Cancelled,
    Unchanged,
    Damaged,
}

/// Mirrors the `stream` union of Archon.Websocket.v0.WSLogEvent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
    Stderr,
}

/// Mirrors Archon.Websocket.v0.PowerState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerState {
    Running,
    Stopped,
    Starting,
    Stopping,
    Crashed,
}

/// Mirrors Archon.Websocket.v0.FlattenedPowerState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FlattenedPowerState {
    NotReady,
    Starting,
    Running,
    Stopping,
    Idle,
}

/// What a power action is moving the server toward. Mirrors the `target` union of
/// Archon.Websocket.v0.WSStateEvent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PowerTarget {
    Start,
    Stop,
    Restart,
}

/// Mirrors Archon.Websocket.v0.SyncInstallPhase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum SyncInstallPhase {
    Analyzing,
    InstallingPack,
    InstallingLoader,
    Addons,
}

/// Mirrors Archon.Websocket.v0.SyncContentProgress.
#[derive(Debug, Clone, Serialize)]
pub struct SyncContentProgress {
    pub started_at: DateTime<Utc>,
    pub phase: SyncInstallPhase,
    pub percent: f64,
}

/// Mirrors Archon.Websocket.v0.SyncContentError.
#[derive(Debug, Clone, Serialize)]
pub struct SyncContentError {
    pub step: String,
    pub description: String,
}

/// Serializes as `"result": "ok"` or `"result": "err"` next to the event tag. Mirrors
/// Archon.Websocket.v0.WSInstallationResultOkEvent and WSInstallationResultErrEvent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum InstallationResult {
    Ok,
    Err {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
    },
}

/// Mirrors the `install_type` union of Archon.Websocket.v0.InstallProgressFileKey.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallType {
    Install,
    Update,
}

/// A loader or platform. Mirrors the `platform` union of Archon.Websocket.v0.InstallProgressPlatformKey
/// and Archon.Sync.v1.WorldContentPlatform, which take the same values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentPlatform {
    Forge,
    Neoforge,
    Fabric,
    Quilt,
    Paper,
    Purpur,
    Vanilla,
}

/// What an install progress item is installing. Mirrors Archon.Websocket.v0.InstallProgressKey.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum InstallProgressKey {
    /// Mirrors Archon.Websocket.v0.InstallProgressFileKey.
    File {
        install_type: InstallType,
        project_id: String,
        version_id: String,
        parent_directory: String,
        source_filename: Option<String>,
        target_filename: Option<String>,
    },
    /// Mirrors Archon.Websocket.v0.InstallProgressModrinthModpackKey.
    ModrinthModpack {
        project_id: String,
        version_id: String,
    },
    /// Mirrors Archon.Websocket.v0.InstallProgressLocalModpackKey.
    LocalModpack { filename: String },
    /// Mirrors Archon.Websocket.v0.InstallProgressPlatformKey.
    Platform {
        platform: ContentPlatform,
        platform_version: String,
        game_version: String,
    },
}

/// Mirrors Archon.Websocket.v0.InstallProgressItem.
#[derive(Debug, Clone, Serialize)]
pub struct InstallProgressItem {
    pub world_id: Uuid,
    pub key: InstallProgressKey,
    pub id: String,
    pub progress: Option<f64>,
    pub error: Option<String>,
}

/// Mirrors Archon.Websocket.v0.FilesystemOpKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FilesystemOpKind {
    Unarchive,
}

/// Mirrors Archon.Websocket.v0.FilesystemOpState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilesystemOpState {
    Queued,
    Ongoing,
    Done,
    Cancelled,
    FailureCorrupted,
    FailureInvalidPath,
}

/// A long file operation, such as an unarchive. Mirrors Archon.Websocket.v0.FilesystemOperation.
#[derive(Debug, Clone, Serialize)]
pub struct FilesystemOperation {
    pub op: FilesystemOpKind,
    pub id: String,
    pub progress: f64,
    pub bytes_processed: u64,
    pub files_processed: u64,
    pub state: FilesystemOpState,
    pub mime: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_file: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invalid_path: Option<String>,
    pub src: String,
    pub started: DateTime<Utc>,
}

/// Query of GET `/v1/sync`. `scope` is `server:<id>`; `intent` is `all` or a comma-separated list
/// of `backup`, `users`, `server`, `protocol`, and `world`. Mirrors the params the api-client's
/// `GenericSyncClient` sends, built from Archon.Sync.v1.SyncIntent.
#[derive(Debug, Clone, Deserialize)]
pub struct SyncQuery {
    pub scope: String,
    pub intent: String,
}

/// One event of the `/v1/sync` stream. Mirrors Archon.Sync.v1.SyncEvent.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type")]
pub enum SyncEvent {
    /// Mirrors Archon.Sync.v1.ProtocolResetEvent.
    #[serde(rename = "protocol.reset")]
    ProtocolReset,
    /// Mirrors Archon.Sync.v1.ProtocolInvalidEvent.
    #[serde(rename = "protocol.invalid")]
    ProtocolInvalid,
    /// Mirrors Archon.Sync.v1.ProtocolErrorEvent.
    #[serde(rename = "protocol.error")]
    ProtocolError { error: String },
    /// Mirrors Archon.Sync.v1.BackupNewEvent.
    #[serde(rename = "backup.new")]
    BackupNew { id: Uuid },
    /// Mirrors Archon.Sync.v1.BackupPatchEvent.
    #[serde(rename = "backup.patch")]
    BackupPatch {
        world_id: Uuid,
        backup_id: Uuid,
        name: String,
    },
    /// Mirrors Archon.Sync.v1.BackupDeleteEvent.
    #[serde(rename = "backup.delete")]
    BackupDelete { world_id: Uuid, backup_id: Uuid },
    /// Mirrors Archon.Sync.v1.BackupOperationStartEvent.
    #[serde(rename = "backup.operation.create.init")]
    BackupCreateInit(BackupOperationStart),
    /// Mirrors Archon.Sync.v1.BackupOperationStartEvent.
    #[serde(rename = "backup.operation.create.start")]
    BackupCreateStart(BackupOperationStart),
    /// Mirrors Archon.Sync.v1.BackupOperationDoneEvent.
    #[serde(rename = "backup.operation.create.done")]
    BackupCreateDone(BackupOperationDone),
    /// Mirrors Archon.Sync.v1.BackupOperationStartEvent.
    #[serde(rename = "backup.operation.restore.init")]
    BackupRestoreInit(BackupOperationStart),
    /// Mirrors Archon.Sync.v1.BackupOperationStartEvent.
    #[serde(rename = "backup.operation.restore.start")]
    BackupRestoreStart(BackupOperationStart),
    /// Mirrors Archon.Sync.v1.BackupOperationDoneEvent.
    #[serde(rename = "backup.operation.restore.done")]
    BackupRestoreDone(BackupOperationDone),
    /// Mirrors Archon.Sync.v1.ServerPatchEvent.
    #[serde(rename = "server.patch")]
    ServerPatch { name: String, subdomain: String },
    /// Mirrors Archon.Sync.v1.ServerNetworkPatchEvent.
    #[serde(rename = "server.network.patch")]
    ServerNetworkPatch { ports: Vec<ServerNetworkPort> },
    /// Mirrors Archon.Sync.v1.UsersPatchEvent.
    #[serde(rename = "users.patch")]
    UsersPatch,
    /// Mirrors Archon.Sync.v1.WorldPatchEvent.
    #[serde(rename = "world.patch")]
    WorldPatch { world_id: Uuid, name: String },
    /// Mirrors Archon.Sync.v1.WorldStartupPatchEvent.
    #[serde(rename = "world.startup.patch")]
    WorldStartupPatch {
        world_id: Uuid,
        java_version: Option<u32>,
        invocation: Option<String>,
        original_invocation: Option<String>,
    },
    /// Mirrors Archon.Sync.v1.WorldContentAddonPatchEvent.
    #[serde(rename = "world.content.addon.patch")]
    WorldContentAddonPatch { world_id: Uuid, specs: Vec<Addon> },
    /// Mirrors Archon.Sync.v1.WorldContentBaseUpdateEvent.
    #[serde(rename = "world.content.base.update")]
    WorldContentBaseUpdate { world_id: Uuid, spec: Addons },
    /// Mirrors Archon.Sync.v1.WorldContentUpdateEvent.
    #[serde(rename = "world.content.update")]
    WorldContentUpdate {
        world_id: Uuid,
        platform_data: Option<WorldContentPlatformData>,
        linked_modpack: Option<WorldContentModpack>,
        #[serde(skip_serializing_if = "Option::is_none")]
        installing: Option<Installing>,
        #[serde(skip_serializing_if = "Option::is_none")]
        error: Option<ContentError>,
        content: Vec<WorldContentItem>,
    },
}

/// Mirrors Archon.Sync.v1.BackupOperationStartEvent, without its `type`.
#[derive(Debug, Clone, Serialize)]
pub struct BackupOperationStart {
    pub world_id: Uuid,
    pub backup_id: Uuid,
    pub operation_id: i64,
}

/// Mirrors Archon.Sync.v1.BackupOperationDoneEvent, without its `type`.
#[derive(Debug, Clone, Serialize)]
pub struct BackupOperationDone {
    pub world_id: Uuid,
    pub backup_id: Uuid,
    pub operation_id: i64,
    pub status: BackupOperationStatus,
}

/// Mirrors Archon.Sync.v1.BackupOperationStatus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BackupOperationStatus {
    Completed,
    Cancelled,
    Failed,
    TimedOut,
}

/// Mirrors Archon.Sync.v1.ServerNetworkPort.
#[derive(Debug, Clone, Serialize)]
pub struct ServerNetworkPort {
    pub port: u16,
    pub name: String,
}

/// Mirrors Archon.Sync.v1.WorldContentPlatformData.
#[derive(Debug, Clone, Serialize)]
pub struct WorldContentPlatformData {
    pub platform: ContentPlatform,
    pub game_version: String,
    pub platform_version: Option<String>,
}

/// Serializes as `{"Modrinth": {...}}` or `{"LocalMrPackFile": {...}}`. Mirrors
/// Archon.Sync.v1.WorldContentModpackSource.
#[derive(Debug, Clone, Serialize)]
pub enum WorldContentModpackSource {
    Modrinth {
        version_id: String,
        project_id: String,
        mrpack_sha1: Option<String>,
    },
    LocalMrPackFile {
        path: String,
        name: String,
        description: Option<String>,
        version_name: Option<String>,
    },
}

/// Mirrors Archon.Sync.v1.WorldContentModpack.
#[derive(Debug, Clone, Serialize)]
pub struct WorldContentModpack {
    pub spec: WorldContentModpackSource,
    pub downloads: Option<u64>,
    pub followers: Option<u64>,
    pub icon_url: Option<String>,
    pub owner: Option<ContentOwner>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub version_number: Option<String>,
    pub date_published: Option<DateTime<Utc>>,
    pub environment: Option<Environment>,
    pub has_update: Option<String>,
}

/// Mirrors Archon.Sync.v1.WorldContentItem.
#[derive(Debug, Clone, Serialize)]
pub struct WorldContentItem {
    pub parent_directory: String,
    pub file_sha1: Option<String>,
    pub filename: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub btime: Option<DateTime<Utc>>,
    pub from_modpack: bool,
    pub version_id: Option<String>,
    pub project_id: Option<String>,
    pub pack_client_retained: bool,
    pub pack_client_depends: bool,
    pub status: AddonStatus,
    pub filesize: Option<u64>,
    pub name: Option<String>,
    pub version: Option<AddonVersion>,
    pub owner: Option<ContentOwner>,
    pub has_update: Option<String>,
    pub icon_url: Option<String>,
}
