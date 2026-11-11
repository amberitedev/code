//! A server's state, in the engine's layout (`packages/engine/src/state/instances/`): `model/` for
//! plain types, `adapters/` for SQLite rows, the server folder and the network, `domain/` for
//! pure rules, and `commands/` with one operation per file. [`Content`] is what every content
//! command runs with.

pub mod adapters;
pub mod commands;
mod content;
pub mod domain;
pub mod model;

pub use self::content::*;
pub use model::*;
