//! What a client sends on a Core's `/events` socket. The Core answers with `CoreEvent`s and the
//! engine's events for the topics it subscribed to.

use serde::{Deserialize, Serialize};

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

#[cfg(test)]
pub(super) fn ts_types() -> Vec<String> {
    use ts_rs::TS;

    let cfg = crate::ts::config();
    vec![Topic::decl(&cfg), ClientMessage::decl(&cfg)]
}
