//! Who can do what: tickets, sessions, per-server roles, the local socket, the activity log, and
//! backend sync. Step 3 builds it; the request and response shapes are in `api::access`,
//! `api::sessions`, `api::activity`, and `api::backend`.

pub mod pairing;
