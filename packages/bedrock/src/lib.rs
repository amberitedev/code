//! The engine's content schema, borrowed in place with `#[path]` ([`state`]) so the engine imports
//! the types from here instead of compiling the files itself.

mod error;
pub mod state;

pub use error::{Error, ErrorKind, Result};
