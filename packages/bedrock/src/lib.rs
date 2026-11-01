//! What the App and Copal both need that is not sharing. Three kinds of code live here:
//!
//! 1. Borrowed app-lib files, compiled in place with `#[path]` ([`state`]). app-lib imports the
//!    types from here instead of compiling the files itself.
//! 2. Clones of upstream code too tangled with app-lib's `State` to borrow ([`upstream`]), each
//!    declared in `ports.toml` with the upstream commit it came from.
//! 3. Our own shared code: content rules ([`content`]), reaching Cores ([`net`]), free disk space
//!    ([`disk`]), and [`limits`].

pub mod disk;
mod error;
pub mod limits;
pub mod net;
pub mod state;

pub use error::{Error, ErrorKind, Result};
