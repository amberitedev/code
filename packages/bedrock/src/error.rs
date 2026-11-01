//! bedrock's error. `Error` is `anyhow::Error`, as in app-lib, so the borrowed and cloned files
//! return the same `crate::Result` there and here, and app-lib uses their results as they are.

pub type Error = anyhow::Error;
pub type Result<T> = core::result::Result<T, Error>;

/// The kinds the borrowed and cloned files raise, worded as app-lib words them.
#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
    #[error("invalid input: {0}")]
    InputError(String),
    #[error("{0}")]
    ContentStoreError(String),
}
