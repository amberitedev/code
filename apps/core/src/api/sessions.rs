//! Sessions: one per device per Core, made from a ticket.
//!
//! - `POST /sessions`: sign in. [`SignIn`] in, [`SignedIn`] out. Limited to 10 a minute per
//!   address.
//! - `GET /sessions`: the owner sees every session, anyone else their own. `Vec<Session>`.
//! - `DELETE /sessions/{id}`: revoke a session. Dangerous when it is someone else's.
//! - `DELETE /sessions/current`: sign out of this Core.
//!
//! The token goes in `Authorization: Bearer` on every other route. Core stores only its hash.

#![allow(dead_code)] // Wired in step 3: sessions.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::access::User;

#[derive(Debug, Clone, Deserialize, TS)]
pub struct SignIn {
    /// A ticket the backend signed for this Core, under five minutes old and never used.
    pub ticket: String,
    /// Up to 64 characters.
    pub device_name: String,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct SignedIn {
    /// 256 random bits. Kept in the OS keychain by the App's Rust side; the WebView never sees
    /// it.
    pub token: String,
    pub session: Session,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct Session {
    /// Public id for listing and revoking. Not the token.
    pub id: String,
    pub user: User,
    pub device_name: String,
    /// Unix seconds.
    pub created_at: i64,
    /// Unix seconds, written at most once an hour.
    pub last_used_at: i64,
    /// Unix seconds. The session ends at this time or 30 days after `last_used_at`, whichever
    /// is first.
    pub expires_at: i64,
    /// The session making this request.
    pub current: bool,
}
