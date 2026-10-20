//! Live updates: the console socket `GET /servers/{id}/ws`, the `GET /v1/sync` event stream, and
//! `POST /servers/{id}/v1/logs/clear` (no body, no response). Mirrors `Archon.Websocket.v0` and
//! `Archon.Sync.v1` in `packages/api-client/src/modules/archon/types.ts`.

#![allow(dead_code)] // The sync stream and most events are wired with the steps that make them.

use std::sync::Arc;

use axum::{
    extract::{
        Path, State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{IntoResponse, Response},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

use super::{
    AppState,
    activity::Action,
    content::{
        Addon, AddonStatus, AddonVersion, Addons, ContentError, ContentOwner,
        Environment, Installing,
    },
};
use crate::{
    access::{Caller, Changed, Level, StreamGuard, activity, now, sessions},
    api::error::{ApiError, ErrorCode},
    servers::console,
};

/// Console lines a new console socket starts with.
const BACKLOG: usize = 200;
/// How often an open console checks that its session has not expired.
const RECHECK_EVERY: std::time::Duration = std::time::Duration::from_secs(600);

/// `GET /servers/{id}/ws`: the console. Sends the last lines and every new one as `log` events
/// and passes `command` messages to the server. Editors and the owner only: it carries chat and
/// player IPs. The upgrade request carries the session; the first message must still be `auth`,
/// as the hosting API has it, but its token is not read. The stream closes when its session ends
/// or its user loses the role, and when the session expires.
pub async fn console(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Before the role check, so a change from here on is never missed; the socket checks again
    // before it shows anything, for the changes before.
    let changes = state.access.changed.subscribe();
    if let Err(error) = caller.require(&state.db, &id, Level::Editor).await {
        return error.into_response();
    }
    let Some(guard) = state.access.open_stream(&caller) else {
        return ApiError::new(
            ErrorCode::RateLimited,
            "This session has too many consoles open.",
        )
        .into_response();
    };
    upgrade.on_upgrade(move |socket| {
        console_socket(state, caller, guard, changes, id, socket)
    })
}

/// Whether `caller` may still read `server`'s console: their session is live and their role
/// still allows it.
async fn still_allowed(
    state: &AppState,
    caller: &Caller,
    server: &str,
) -> bool {
    if let Caller::Session { id, .. } = caller {
        let exists = sqlx::query(
            "SELECT 1 FROM sessions WHERE id = ?1 AND expires_at > ?2 AND last_used_at > ?2 - ?3",
        )
        .bind(id)
        .bind(now())
        .bind(sessions::IDLE_S)
        .fetch_optional(&state.db)
        .await;
        if !matches!(exists, Ok(Some(_))) {
            return false;
        }
    }
    matches!(
        caller.level(&state.db, server).await,
        Ok(Some(level)) if level >= Level::Editor
    )
}

async fn console_socket(
    state: Arc<AppState>,
    caller: Caller,
    _guard: StreamGuard,
    mut changes: broadcast::Receiver<Changed>,
    id: String,
    mut socket: WebSocket,
) {
    let first = socket.recv().await;
    let authed = matches!(
        first,
        Some(Ok(Message::Text(text)))
            if matches!(serde_json::from_str(&text), Ok(WsClientMessage::Auth { .. }))
    );
    if !authed
        || !still_allowed(&state, &caller, &id).await
        || send(&mut socket, &WsEvent::AuthOk).await.is_err()
    {
        return;
    }
    let mut recheck = tokio::time::interval(RECHECK_EVERY);
    recheck.tick().await;
    let path = state.servers.paths().console_log(&id);
    let (backlog, end) = console::tail(&path, BACKLOG).unwrap_or_default();
    for line in backlog {
        if send(&mut socket, &log(line)).await.is_err() {
            return;
        }
    }
    let (sender, mut lines) = mpsc::channel(256);
    console::follow(path, end, sender);
    let mut installs = state.content.events.subscribe();
    loop {
        tokio::select! {
            event = installs.recv() => {
                if let Ok((server, event)) = event
                    && server == id
                    && send(&mut socket, &event).await.is_err()
                {
                    return;
                }
            }
            line = lines.recv() => {
                let Some(line) = line else { return };
                if send(&mut socket, &log(line)).await.is_err() {
                    return;
                }
            }
            change = changes.recv() => {
                let concerns_me = match change {
                    Ok(Changed::User(user)) => caller.user_id() == Some(user.as_str()),
                    Ok(Changed::Everyone) | Err(_) => true,
                };
                if concerns_me && !still_allowed(&state, &caller, &id).await {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
            _ = recheck.tick() => {
                if !still_allowed(&state, &caller, &id).await {
                    let _ = socket.send(Message::Close(None)).await;
                    return;
                }
            }
            message = socket.recv() => match message {
                Some(Ok(Message::Text(text))) => {
                    if let Ok(WsClientMessage::Command { cmd }) = serde_json::from_str(&text) {
                        match state.servers.command(&id, &cmd).await {
                            Ok(()) => {
                                activity::log(
                                    &state.db,
                                    &caller,
                                    Some(&id),
                                    Action::ConsoleCommand { command: cmd },
                                )
                                .await;
                            }
                            Err(error) => {
                                tracing::debug!(server = %id, %error, "console command");
                            }
                        }
                    }
                }
                Some(Ok(_)) => {}
                Some(Err(_)) | None => return,
            },
        }
    }
}

fn log(message: String) -> WsEvent {
    WsEvent::Log {
        stream: LogStream::Stdout,
        message,
    }
}

async fn send(
    socket: &mut WebSocket,
    event: &WsEvent,
) -> Result<(), axum::Error> {
    let text = serde_json::to_string(event).expect("events serialize");
    socket.send(Message::Text(text.into())).await
}

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
