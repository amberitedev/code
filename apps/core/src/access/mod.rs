//! Who can do what: tickets, sessions, per-server roles, the local socket, the activity log, and
//! backend sync. The local socket is built; step 3 builds the rest. The request and response
//! shapes are in `api::access`, `api::sessions`, `api::activity`, and `api::backend`.

pub mod local;
pub mod pairing;
