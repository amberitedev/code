//! The calls Core owes the backend, which `sync` stores in `backend_owed` and retries until they
//! land (see [`super::sync::owe`]).

use contracts::backend::cores::{
    AccessGrant, AccessRevocation, ForgottenServer,
};
use serde::{Deserialize, Serialize};

/// A call Core still owes the backend, stored as JSON in `backend_owed.call` and retried with
/// backoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OwedCall {
    /// `POST /core/access`.
    Grant(AccessGrant),
    /// `DELETE /core/access`.
    Remove(AccessRevocation),
    /// `DELETE /v1/instances/{share_id}`: the own share of a server that was deleted.
    DeleteShare { share_id: String },
    /// `DELETE /core/players`.
    ForgetPlayers(ForgottenServer),
    /// `POST /core/state` with the state at the time it is sent.
    ReportState,
    /// `POST /core/storage` with the store's holdings at the time it is sent.
    ReportStorage,
}
