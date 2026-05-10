//! Unified error type for Amberite backend

use serde::Serialize;
use thiserror::Error;

/// All errors that can occur in Amberite backend operations
#[derive(Error, Debug, Serialize, Clone)]
pub enum AmberiteError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(String),

    #[error("Serialization error: {0}")]
    Serialization(String),

    #[error("HTTP error: {0}")]
    Http(String),

    #[error("WebSocket error: {0}")]
    WebSocket(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Core not connected")]
    CoreNotConnected,

    #[error("Core error: {0}")]
    Core(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Not implemented")]
    NotImplemented,

    #[error("Tunnel error: {0}")]
    Tunnel(String),

    #[error("{0}")]
    Other(String),
}

impl From<std::io::Error> for AmberiteError {
    fn from(e: std::io::Error) -> Self {
        AmberiteError::Io(e.to_string())
    }
}

impl From<serde_json::Error> for AmberiteError {
    fn from(e: serde_json::Error) -> Self {
        AmberiteError::Serialization(e.to_string())
    }
}

impl From<reqwest::Error> for AmberiteError {
    fn from(e: reqwest::Error) -> Self {
        AmberiteError::Http(e.to_string())
    }
}

/// Result alias for Amberite operations
pub type Result<T> = std::result::Result<T, AmberiteError>;
