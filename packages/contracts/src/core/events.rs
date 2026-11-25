//! A Core's `/events` socket: what a client sends, and the [`CoreEvent`]s the Core answers with
//! beside the engine's events for the topics it subscribed to.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// A stream of events a socket can subscribe to.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Topic {
    /// The engine's instance and process events for one instance.
    Instance { id: String },
    /// The engine's install jobs, for every instance the caller may see, starting with the
    /// unfinished ones.
    Installs,
    /// A server's console lines, as the engine's `log` events, starting with its last 200.
    Console { id: String },
    /// A server's power state, starting with the current one.
    Server { id: String },
    /// A server's backup progress.
    Backups { id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    Subscribe { topic: Topic },
    Unsubscribe { topic: Topic },
}

/// What the Core sends on `/events`, beside the engine's events. A console line is the engine's
/// `log` event, the same as an App instance's. The payloads are Modrinth Hosting's server socket
/// events (`Archon.Websocket.v0`), with the server they are about. A topic of a missing or removed
/// server gets no events; the server's routes answer `not_found`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    PowerState {
        server: String,
        state: PowerState,
    },
    /// How many seconds a running server has been up.
    Uptime {
        server: String,
        uptime: u64,
    },
    /// Times are Unix seconds: numbers, as Kyros sends them.
    BackupProgress {
        server: String,
        id: Uuid,
        task: BackupTask,
        state: BackupTaskState,
        progress: f64,
        start_time: Option<i64>,
        finish_time: Option<i64>,
    },
    /// This socket fell behind and lost events: refetch what is shown.
    Resync,
}

/// Mirrors Archon.Websocket.v0.PowerState.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum PowerState {
    Running,
    Stopped,
    Starting,
    Stopping,
    Crashed,
}

/// Mirrors Archon.Websocket.v0.BackupTask.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum BackupTask {
    Create,
    Restore,
}

/// Mirrors Archon.Websocket.v0.BackupState.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum BackupTaskState {
    Ongoing,
    Done,
    Failed,
}

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![
        Topic::decl(&cfg),
        ClientMessage::decl(&cfg),
        CoreEvent::decl(&cfg),
        PowerState::decl(&cfg),
        BackupTask::decl(&cfg),
        BackupTaskState::decl(&cfg),
    ]
}
