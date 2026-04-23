//! Theseus error types
//! Simplified from the full Theseus error system

use thiserror::Error;

pub type TheseusResult<T> = Result<T, TheseusError>;

#[derive(Error, Debug)]
pub enum TheseusError {
    #[error("IO error: {0}")]
    Io(String),
    
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    
    #[error("Zip error: {0}")]
    Zip(String),
    
    #[error("Fetch error: {0}")]
    Fetch(String),
    
    #[error("Hash mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    
    #[error("Invalid modpack: {0}")]
    InvalidModpack(String),
    
    #[error("Modpack not found: {0}")]
    ModpackNotFound(String),
    
    #[error("Download failed: {0}")]
    DownloadFailed(String),
    
    #[error("Profile error: {0}")]
    Profile(String),
    
    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),
    
    #[error("Other error: {0}")]
    Other(String),
}

impl From<std::io::Error> for TheseusError {
    fn from(e: std::io::Error) -> Self {
        TheseusError::Io(e.to_string())
    }
}

impl From<async_zip::error::ZipError> for TheseusError {
    fn from(e: async_zip::error::ZipError) -> Self {
        TheseusError::Zip(e.to_string())
    }
}

impl From<reqwest::Error> for TheseusError {
    fn from(e: reqwest::Error) -> Self {
        TheseusError::Fetch(e.to_string())
    }
}
