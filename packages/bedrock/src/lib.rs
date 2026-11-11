//! What the App and Copal both need that is not sharing. Three kinds of code live here:
//!
//! 1. Borrowed app-lib files, compiled in place with `#[path]` ([`state`]). app-lib imports the
//!    types from here instead of compiling the files itself.
//! 2. Clones of upstream code too tangled with app-lib's `State` to borrow ([`upstream`]), each
//!    declared in `ports.toml` with the upstream commit it came from.
//! 3. Our own shared code: content rules ([`content`]).

pub mod content;
mod error;
pub mod state;
pub mod upstream;
mod util;

// The content store clone finds these at the crate root, where app-lib has them.
pub(crate) use upstream::store::{State, install};

pub use error::{Error, ErrorKind, Result};
