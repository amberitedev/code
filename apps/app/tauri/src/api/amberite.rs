use crate::api::Result;
use serde::Serialize;
use tauri::plugin::TauriPlugin;
use tauri::Runtime;
use thiserror::Error;

/// Error type for Amberite commands (separate from TheseusSerializableError to avoid merge conflicts)
#[derive(Error, Debug, Serialize, Clone)]
pub enum AmberiteCommandError {
    #[error("{0}")]
    Amberite(String),
}

impl From<amberite_backend::AmberiteError> for AmberiteCommandError {
    fn from(e: amberite_backend::AmberiteError) -> Self {
        AmberiteCommandError::Amberite(e.to_string())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    tauri::plugin::Builder::<R>::new("amberite")
        .invoke_handler(tauri::generate_handler![hello])
        .build()
}

#[tauri::command]
pub async fn hello() -> Result<String> {
    let placeholder = amberite_backend::get_placeholder()
        .map_err(|e| theseus::Error::from(theseus::ErrorKind::OtherError(e.to_string())))?;
    Ok(placeholder.message)
}
