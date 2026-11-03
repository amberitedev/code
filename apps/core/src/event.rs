//! The events Core sends on a server's console socket (`api::events`), as app-lib keeps its
//! events in `crate::event`. Mirrors `Archon.Websocket.v0` in
//! `packages/api-client/src/modules/archon/types.ts`, with only the events Core sends. Backups and
//! content installs report on them.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

// What installs report, as `state::servers::commands` produce it.
pub use crate::state::servers::model::{GameVersionChange, InstallProgressKey};

/// A message the App sends on the console socket. Mirrors Archon.Websocket.v0.WSOutgoingMessage.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum WsClientMessage {
    /// Mirrors Archon.Websocket.v0.WSAuthMessage.
    Auth {},
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
    /// Mirrors Archon.Websocket.v0.WSAuthOkEvent.
    AuthOk,
    /// Mirrors Archon.Websocket.v0.WSPowerStateEvent. Core does not report exit codes here.
    PowerState { state: PowerState },
    /// Mirrors Archon.Websocket.v0.WSUptimeEvent: seconds the running server has been up.
    Uptime { uptime: u64 },
    /// Mirrors Archon.Websocket.v0.WSInstallationResultEvent.
    InstallationResult(InstallationResult),
    /// Mirrors Archon.Websocket.v0.WSInstallProgressEvent.
    InstallProgress { items: Vec<InstallProgressItem> },
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

/// Mirrors Archon.Websocket.v0.BackupTask.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupTask {
    Create,
    Restore,
}

/// Mirrors Archon.Websocket.v0.BackupState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum BackupState {
    Ongoing,
    Done,
    Failed,
}

/// Mirrors the `stream` union of Archon.Websocket.v0.WSLogEvent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum LogStream {
    Stdout,
}

/// Serializes as `"result": "ok"` or `"result": "err"` next to the event tag. Mirrors
/// Archon.Websocket.v0.WSInstallationResultOkEvent and WSInstallationResultErrEvent, with
/// Amberite's `game_version` on a pack install that moved the server to another Minecraft.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "result", rename_all = "lowercase")]
pub enum InstallationResult {
    Ok {
        #[serde(skip_serializing_if = "Option::is_none")]
        game_version: Option<GameVersionChange>,
    },
    Err {
        #[serde(skip_serializing_if = "Option::is_none")]
        reason: Option<String>,
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
