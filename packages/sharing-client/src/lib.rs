//! What the App and Core share about sharing. Laid out like app-lib,
//! one folder per concern:
//!
//! - `backend`: the client for the backend's share routes.
//! - `versions`: the share version as the backend gives and takes it.
//! - `rules`: which files a share may carry, and what a blocked file is.
//! - `push`: the writer of a share's versions.
//! - `pool`: the client for Cores' stores, and fetching a version's files from them.
//! - `consume`: a version as a server takes it.
//! - `install`: a version as a member installs it, beside or on top of its modpack.
//! - `diff`: what one share content changes over another, for updates, pushes and history.
//! - `history`: the versions an owner pushed, kept locally by the side that pushed them.
//! - `crypto`: the pool's encryption.

pub mod backend;
pub mod consume;
pub mod crypto;
pub mod diff;
pub mod history;
pub mod install;
pub mod pool;
pub mod push;
pub mod rules;
pub mod versions;
