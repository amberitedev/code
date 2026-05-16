use std::sync::Arc;

use crate::{application::state::AppState, domain::instance::InstanceId};

pub type MacroPid = u64;

#[derive(Debug, thiserror::Error)]
pub enum MacroError {
    #[error("instance not found: {0}")]
    InstanceNotFound(InstanceId),
    #[error("macro file not found: {0}")]
    FileNotFound(String),
    #[error("macro not found: pid {0}")]
    MacroNotFound(MacroPid),
    #[error("invalid macro name")]
    InvalidName,
    #[error("macro runtime is disabled")]
    Disabled,
}

/// SEC-06: Spawn a macro for an instance by name. Returns the macro PID.
pub fn spawn_macro(
    state: &Arc<AppState>,
    instance_id: InstanceId,
    macro_name: String,
) -> Result<MacroPid, MacroError> {
    if macro_name.is_empty()
        || macro_name.contains("..")
        || macro_name.contains('/')
        || macro_name.contains('\\')
    {
        return Err(MacroError::InvalidName);
    }
    if !state.instances.contains_key(&instance_id) {
        return Err(MacroError::InstanceNotFound(instance_id));
    }

    let macro_dir = state
        .config
        .data_dir
        .join("instances")
        .join(instance_id.to_string())
        .join("macros");
    let ts_path = macro_dir.join(&macro_name).with_extension("ts");
    let js_path = macro_dir.join(&macro_name).with_extension("js");

    let path = if ts_path.exists() {
        ts_path
    } else if js_path.exists() {
        js_path
    } else {
        return Err(MacroError::FileNotFound(macro_name));
    };

    let _path = path;
    Err(MacroError::Disabled)
}

/// Kill a macro by PID.
pub fn kill_macro(
    _state: &Arc<AppState>,
    pid: MacroPid,
) -> Result<(), MacroError> {
    Err(MacroError::MacroNotFound(pid))
}

/// List all running macro PIDs for a given instance.
pub fn list_macros(_state: &Arc<AppState>) -> Vec<MacroPid> {
    Vec::new()
}

/// List available macro files for an instance.
pub async fn list_macro_files(
    state: &Arc<AppState>,
    instance_id: &InstanceId,
) -> Vec<String> {
    let macro_dir = state
        .config
        .data_dir
        .join("instances")
        .join(instance_id.to_string())
        .join("macros");
    let Ok(mut entries) = tokio::fs::read_dir(&macro_dir).await else {
        return Vec::new();
    };
    let mut names = Vec::new();
    while let Ok(Some(entry)) = entries.next_entry().await {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.ends_with(".ts") || name.ends_with(".js") {
            names.push(name);
        }
    }
    names
}
