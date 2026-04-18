//! Amberite API - Custom commands that extend/modify Theseus behavior
use crate::api::Result;
use amberite_backend::Placeholder;
use serde::{Deserialize, Serialize};

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("amberite")
        .invoke_handler(tauri::generate_handler![
            amberite_hello,
            amberite_get_version,
        ])
        .build()
}

/// Simple hello command to verify Amberite backend is working
#[tauri::command]
pub async fn amberite_hello() -> Result<String> {
    Ok("Hello from Amberite!".to_string())
}

/// Get Amberite version and placeholder data
#[tauri::command]
pub async fn amberite_get_version() -> Result<AmberiteVersionInfo> {
    let placeholder = amberite_backend::get_placeholder()
        .map_err(|e| crate::api::TheseusSerializableError::Theseus(theseus::Error::from(theseus::ErrorKind::OtherError(e.to_string()))))?;
    
    Ok(AmberiteVersionInfo {
        version: env!("CARGO_PKG_VERSION").to_string(),
        placeholder_message: placeholder.message,
    })
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AmberiteVersionInfo {
    pub version: String,
    pub placeholder_message: String,
}
