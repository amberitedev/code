//! The engine's content model files (`packages/engine/src/state/instances/model/`), compiled in
//! place. The engine imports these types from here instead of compiling the files itself.

#[path = "../../../../../engine/src/state/instances/model/content_entry.rs"]
mod content_entry;
pub use self::content_entry::*;

#[path = "../../../../../engine/src/state/instances/model/content_set.rs"]
mod content_set;
pub use self::content_set::*;

#[path = "../../../../../engine/src/state/instances/model/content_set_sync_state.rs"]
mod content_set_sync_state;
pub use self::content_set_sync_state::*;

#[path = "../../../../../engine/src/state/instances/model/file.rs"]
mod file;
pub use self::file::*;

#[path = "../../../../../engine/src/state/instances/model/manifest.rs"]
mod manifest;
pub use self::manifest::*;

#[path = "../../../../../engine/src/state/instances/model/link.rs"]
mod link;
pub use self::link::*;

/// The error the borrowed `from_str` parsers return, as the engine's `unknown_value` builds it.
fn unknown_value(kind: &str, value: &str) -> crate::Error {
    crate::ErrorKind::InputError(format!("unknown {kind} {value}")).into()
}
