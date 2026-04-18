//! Error types for Amberite backend

use thiserror::Error;

/// Errors that can occur in Amberite backend
#[derive(Error, Debug)]
pub enum BackendError {
    #[error("Configuration error: {0}")]
    Config(String),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    
    #[error("Async error: {0}")]
    Async(String),
}

pub type Result<T> = std::result::Result<T, BackendError>;
