//! What Core and the backend exchange. The backend is a phone book and a notifier: Core reports
//! on start and on change, never on a timer, and is the only authority on access.
//!
//! - Tickets: the App gets a [`TicketClaims`] JWT from the backend (`POST /cores/{id}/ticket`)
//!   and hands it to one Core.
//! - `GET /core/keys.json` (a static file): the backend's ticket keys, [`PublicKeys`].
//! - `POST /core/state`: on start, on reconnect, and when the address changes. [`StateReport`].
//! - `PUT /core/access`: the whole directory, replacing the backend's copy, on every start and
//!   reconnect. [`AccessDirectory`].
//! - `POST /core/access`: this user may reach this Core; notify them. [`Grant`].
//! - `DELETE /core/access`: withdraw the invite, and the directory entry unless `keep_core`.
//!   [`Remove`].
//!
//! Every call is authenticated with Core's own key, `Authorization: Bearer amc_...`. A key the
//! backend no longer knows gets 410 `core_removed` from the state report and the directory
//! replace: the Core was removed from its account and un-pairs.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::access::Role;

/// The claims of a ticket: an Ed25519-signed JWT (`alg: EdDSA`), valid for five minutes, for one
/// Core and one use.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct TicketClaims {
    /// The backend's issuer name.
    pub iss: String,
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

/// The backend's ticket keys as a JWK set. A key stays listed for a day after it is replaced.
#[derive(Debug, Clone, Default, Serialize, Deserialize, TS)]
pub struct PublicKeys {
    pub keys: Vec<PublicKey>,
}

/// One Ed25519 public key as a JWK.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct PublicKey {
    /// Always `OKP`.
    pub kty: String,
    /// Always `Ed25519`.
    pub crv: String,
    /// The id a ticket names in its `kid` header.
    pub kid: String,
    /// The key, base64url.
    pub x: String,
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

/// A call Core still owes the backend, stored as JSON in `backend_owed.call` and retried with
/// backoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OwedCall {
    /// `POST /core/access`.
    Grant(Grant),
    /// `DELETE /core/access`.
    Remove(Remove),
    /// `POST /core/state` with the state at the time it is sent.
    ReportState,
}
