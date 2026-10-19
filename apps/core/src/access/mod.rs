//! Who can do what: tickets, sessions, per-server roles, the local socket, the activity log, and
//! backend sync. The local socket is built; step 3 builds the rest. The request and response
//! shapes are in `api::access`, `api::sessions`, `api::activity`, and `api::backend`.

#![allow(dead_code)] // Wired with sessions and dangerous actions.

pub mod keys;
pub mod local;
pub mod pairing;
pub mod tickets;

/// Unix seconds.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}
