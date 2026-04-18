//! Error types for Amberite Tauri commands

use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};
use thiserror::Error;

pub type Result<T> = std::result::Result<T, AmberiteSerializableError>;

/// Serializable error for Amberite commands
#[derive(Error, Debug)]
pub enum AmberiteSerializableError {
    #[error("{0}")]
    Amberite(#[from] amberite_backend::AmberiteError),
    
    #[error("Theseus error: {0}")]
    Theseus(#[from] theseus::Error),
    
    #[error("IO error: {0}")]
    IO(#[from] std::io::Error),
}

impl Serialize for AmberiteSerializableError {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        match self {
            AmberiteSerializableError::Amberite(e) => {
                let mut state = serializer.serialize_struct("Amberite", 2)?;
                state.serialize_field("field_name", "Amberite")?;
                state.serialize_field("message", &e.to_string())?;
                state.end()
            }
            AmberiteSerializableError::Theseus(e) => {
                let mut state = serializer.serialize_struct("Theseus", 2)?;
                state.serialize_field("field_name", "Theseus")?;
                state.serialize_field("message", &e.to_string())?;
                state.end()
            }
            AmberiteSerializableError::IO(e) => {
                let mut state = serializer.serialize_struct("IO", 2)?;
                state.serialize_field("field_name", "IO")?;
                state.serialize_field("message", &e.to_string())?;
                state.end()
            }
        }
    }
}
