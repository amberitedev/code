//! What the App and Core share about sharing. Laid out like app-lib,
//! one folder per concern:
//!
//! - `shares`: what both read the same way from the backend's share routes.
//! - `versions`: a share version as both read it.
//! - `rules`: which files a share may carry, and what a blocked file is.
//! - `push`: the writer of a share's versions.
//! - `pool`: the client for Cores' stores, and fetching a version's files from them.
//! - `install`: a version as a member installs it, beside or on top of its modpack.
//! - `diff`: what one share content changes over another, for updates, pushes and history.
//! - `history`: the versions an owner pushed, kept locally by the side that pushed them.
//! - `crypto`: the pool's encryption.
//! - `net`: reaching a Core.
//! - `disk`: free disk space, checked before the store and Core write.

pub mod crypto;
pub mod diff;
pub mod disk;
pub mod history;
pub mod install;
pub mod net;
pub mod pool;
pub mod push;
pub mod rules;
pub mod shares;
pub mod versions;
