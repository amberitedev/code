//! Routes the backend serves.

use crate::Empty;

pub mod accounts;
pub mod cores;
pub mod dev;
pub mod sharing;
pub mod social;

crate::endpoints! {
    /// The backend's routes outside accounts, cores, dev, sharing and social, which have their own consts.
    group backend;

    /// Answers once the backend reaches its database.
    CheckHealth: Get "/health", None, (), (), Empty => Empty;
}

#[cfg(test)]
pub(crate) fn ts() -> String {
    ts_module().ts(false)
}
