//! What Core and the backend exchange. The backend is a phone book and a notifier: Core reports
//! on start and on change, and only storage reports also go on a timer. Core is the only
//! authority on access.
//!
//! - Tickets: the App gets a [`TicketClaims`] JWT from the backend (`POST /cores/{id}/ticket`)
//!   and hands it to one Core.
//! - `GET /core/keys.json` (a static file): the backend's ticket keys,
//!   [`PublicKeys`](sharing_storage::keys::PublicKeys).
//! - `POST /core/state`: on start, on reconnect, and when the address changes. [`StateReport`].
//! - `PUT /core/access`: the whole directory, replacing the backend's copy, on every start and
//!   reconnect. [`AccessDirectory`].
//! - `POST /core/access`: this user may reach this Core; notify them. [`Grant`].
//! - `DELETE /core/access`: withdraw the invite, and the directory entry unless `keep_core`.
//!   [`Remove`].
//! - `POST /core/storage`: what this Core's store holds and has room for, after uploads and pulls
//!   and hourly. [`StorageReport`] in, [`StorageAnswer`] out (see `storage`).
//!
//! Every call is authenticated with Core's own key, `Authorization: Bearer amc_...`. A key the
//! backend no longer knows gets 410 `core_removed` from the state report and the directory
//! replace: the Core was removed from its account and un-pairs.

use serde::{Deserialize, Serialize};
use sharing_storage::local::HeldVersion;
use ts_rs::TS;

use super::access::Role;

/// The claims of a sign-in ticket: an Ed25519-signed JWT (`alg: EdDSA`), valid for five minutes,
/// for one Core and one use. The pool's tickets are signed with the same keys and differ in
/// `kind` (see `sharing_storage`).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TicketClaims {
    /// The backend's issuer name.
    pub iss: String,
    /// Always `session`. Core refuses any other kind for sign-in.
    pub kind: String,
    /// The user id.
    pub sub: String,
    pub name: String,
    pub avatar_url: Option<String>,
    /// The Core id this ticket is for.
    pub aud: String,
    /// Unix seconds.
    pub iat: i64,
    /// Unix seconds, five minutes after `iat`.
    pub exp: i64,
    /// Random one-time id. Core records it in the same write that uses the ticket.
    pub jti: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StateReport {
    pub version: String,
    /// Where the App reaches this Core on its own network, `https://ip:port`.
    pub address: String,
    /// Where the App reaches it from outside, once known.
    pub public_address: Option<String>,
    /// SHA-256 of the TLS certificate, lowercase hex. The App pins it.
    pub cert_fingerprint: String,
    /// The servers shown to the people their instance is shared with. Empty until servers
    /// follow shared instances (step 4).
    pub servers: Vec<VisibleServer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct VisibleServer {
    pub id: String,
    pub name: String,
    /// The Minecraft address, `host:port`.
    pub address: String,
    pub shared_instance_id: String,
}

/// Everyone with a role on any server of this Core: who may be told its address. Nothing about
/// roles; the owner is never listed.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccessDirectory {
    pub user_ids: Vec<String>,
}

/// Body of `POST /core/access`: the owner invited `user` to a server. The backend adds them to
/// the directory and notifies them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Grant {
    /// The user id.
    pub user: String,
    pub server_id: String,
    pub server_name: String,
    pub role: Role,
}

/// Body of `DELETE /core/access`: `user_id` lost `server_id`. `keep_core` when they still have
/// another server on this Core, so they stay in the directory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct Remove {
    pub user_id: String,
    pub server_id: String,
    pub keep_core: bool,
}

/// Body of `DELETE /core/players`: `server_id` was deleted or stopped following an instance, so
/// the backend drops its players and their links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct ForgetPlayers {
    pub server_id: String,
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

/// Body of `POST /core/storage`. Bytes throughout.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct StorageReport {
    /// This Core offers storage to other people's shares. Off, it holds only its owner's.
    pub enabled: bool,
    /// Core is installed as a service.
    pub service: bool,
    pub cap: u64,
    pub used: u64,
    /// Free on the store's disk.
    pub free: u64,
    /// What it may serve a day, and has served today (UTC).
    pub egress_cap: u64,
    pub egress_today: u64,
    /// Every version the store holds whole.
    pub versions: Vec<HeldVersion>,
}

/// What the backend answers a [`StorageReport`] with.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct StorageAnswer {
    /// Roots to stop holding.
    #[serde(default)]
    pub drop: Vec<String>,
    /// Versions to copy from another Core.
    #[serde(default)]
    pub pull: Vec<PullOrder>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PullOrder {
    pub root: String,
    /// A pull ticket naming the source Core.
    pub ticket: String,
}
