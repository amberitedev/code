//! The calls Core owes the backend, which `sync` stores in `backend_owed` and retries until they
//! land (see [`super::sync::owe`]).

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::api::backend::{Grant, Remove};

/// Body of `DELETE /core/players`: `server_id` was deleted or stopped following an instance, so
/// the backend drops its players and their links. With `share_id`, only while the server still
/// follows that share there: a late call never drops the players of a share it follows since.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ForgetPlayers {
    pub server_id: String,
    pub share_id: Option<String>,
}

/// A call Core still owes the backend, stored as JSON in `backend_owed.call` and retried with
/// backoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OwedCall {
    /// `POST /core/access`.
    Grant(Grant),
    /// `DELETE /core/access`.
    Remove(Remove),
    /// `DELETE /v1/instances/{share_id}`: a server's own share, which the server let go of when
    /// it was deleted or linked to an instance.
    DeleteShare { share_id: String },
    /// `DELETE /core/players`.
    ForgetPlayers(ForgetPlayers),
    /// `POST /core/state` with the state at the time it is sent.
    ReportState,
    /// `POST /core/storage` with the store's holdings at the time it is sent.
    ReportStorage,
}
