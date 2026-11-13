//! A Core's `/events` socket: what a client sends, and the [`CoreEvent`]s the Core answers with
//! beside the engine's events for the topics it subscribed to.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ApiError;

/// A stream of events a socket can subscribe to. Viewers and up may subscribe to every topic but
/// `Console`, which needs Editor: it carries chat and player IPs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Topic {
    /// The engine's instance and process events for one instance.
    Instance { id: String },
    /// The engine's install jobs, for every instance the caller may see.
    Installs,
    /// A server's console lines, starting with its last 200.
    Console { id: String },
    /// A server's power state, starting with the current one.
    Server { id: String },
    /// A server's backup progress.
    Backups { id: String },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ClientMessage {
    /// A topic the caller may not see is answered with `Ended`.
    Subscribe {
        topic: Topic,
    },
    Unsubscribe {
        topic: Topic,
    },
}

/// What the Core sends on `/events`, beside the engine's events.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum CoreEvent {
    /// A console line, without Core's health checks.
    Console { server: String, line: String },
    /// `uptime` is how many seconds a running server has been up.
    Power {
        server: String,
        state: PowerState,
        uptime: Option<u64>,
    },
    Backup {
        server: String,
        id: Uuid,
        task: BackupTask,
        state: BackupTaskState,
        progress: f64,
        start_time: Option<i64>,
        finish_time: Option<i64>,
    },
    /// The caller may not, or no longer, see `topic`.
    Ended { topic: Topic, error: ApiError },
    /// This socket fell behind and lost events: refetch what is shown.
    Resync,
}

/// Mirrors Archon.Websocket.v0.PowerState.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum BackupTask {
    Create,
    Restore,
}

/// Mirrors Archon.Websocket.v0.BackupState.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
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
