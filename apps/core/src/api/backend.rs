//! What Core and the backend exchange. The backend is a phone book and a notifier: Core reports
//! on start and on change, never on a timer, and is the only authority on access.
//!
//! - Tickets: the App gets a [`TicketClaims`] JWT from the backend and hands it to one Core.
//! - `GET /core/keys.json` (a static file): the backend's ticket keys, [`PublicKeys`].
//! - `POST /core/state`: once on start, and when the address changes. [`StateReport`].
//! - `PUT /core/access`: the whole directory, replacing the backend's copy, on every start and
//!   reconnect. [`AccessDirectory`].
//! - `POST /core/access/{user_id}`: this user may reach this Core; notify them.
//! - `DELETE /core/access/{user_id}`: drop the directory entry.
//!
//! Every call is authenticated with Core's own key, `Authorization: Bearer amc_...`.

#![allow(dead_code)] // Wired in step 3: tickets and backend sync.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

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
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
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
}

/// Everyone with a role on any server of this Core: who may be told its address. Nothing about
/// roles; the owner is never listed.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct AccessDirectory {
    pub user_ids: Vec<String>,
}

/// A call Core still owes the backend, stored as JSON in `backend_owed.call` and retried with
/// backoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum OwedCall {
    /// `POST /core/access/{user_id}`.
    Grant { user_id: String },
    /// `DELETE /core/access/{user_id}`.
    Drop { user_id: String },
    /// `POST /core/state` with the state at the time it is sent.
    ReportState,
}
