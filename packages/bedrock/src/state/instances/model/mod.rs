//! app-lib's content model files (`packages/app-lib/src/state/instances/model/`), compiled in
//! place. app-lib imports these types from here instead of compiling the files itself.

#[path = "../../../../../app-lib/src/state/instances/model/content_entry.rs"]
mod content_entry;
pub use self::content_entry::*;

#[path = "../../../../../app-lib/src/state/instances/model/content_set.rs"]
mod content_set;
pub use self::content_set::*;

#[path = "../../../../../app-lib/src/state/instances/model/file.rs"]
mod file;
pub use self::file::*;

#[path = "../../../../../app-lib/src/state/instances/model/manifest.rs"]
mod manifest;
pub use self::manifest::*;

/// The error the borrowed `from_str` parsers return, as app-lib's `unknown_value` builds it.
fn unknown_value(kind: &str, value: &str) -> crate::Error {
    crate::ErrorKind::InputError(format!("unknown {kind} {value}")).into()
}
