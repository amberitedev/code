use crate::error::{AmberiteError, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionPreset {
    pub id: String,
    pub name: String,
    pub description: String,
    pub role: String,
    pub permissions: serde_json::Value,
}

fn presets_dir() -> Result<PathBuf> {
    let config = dirs::config_dir().ok_or_else(|| {
        AmberiteError::Config("Cannot find config directory".into())
    })?;
    Ok(config.join("amberite").join("permission-presets"))
}

pub async fn list_permission_presets() -> Result<Vec<PermissionPreset>> {
    let dir = presets_dir()?;
    write_default_presets(&dir).await?;

    let mut entries = tokio::fs::read_dir(&dir).await?;
    let mut presets: Vec<PermissionPreset> = Vec::new();
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
            continue;
        }
        let raw = tokio::fs::read_to_string(&path).await?;
        presets.push(serde_json::from_str(&raw)?);
    }
    presets.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(presets)
}

async fn write_default_presets(dir: &PathBuf) -> Result<()> {
    tokio::fs::create_dir_all(dir).await?;
    for preset in default_presets() {
        let path = dir.join(format!("{}.json", preset.id));
        if !path.exists() {
            let raw = serde_json::to_string_pretty(&preset)?;
            tokio::fs::write(path, raw).await?;
        }
    }
    Ok(())
}

fn default_presets() -> Vec<PermissionPreset> {
    vec![
        preset(
            "owner",
            "Owner",
            "Full Core control; cannot be removed.",
            "owner",
            &["*"],
        ),
        preset(
            "admin",
            "Admin",
            "Manage members, permissions, and server lifecycle.",
            "admin",
            &[
                "core.manage",
                "members.manage",
                "instances.manage",
                "console.write",
            ],
        ),
        preset(
            "member",
            "Member",
            "Play, install synchronized profiles, and view status.",
            "member",
            &["instances.view", "profiles.install", "console.read"],
        ),
        preset(
            "viewer",
            "Viewer",
            "Read-only access for server state and packs.",
            "member",
            &["instances.view", "console.read"],
        ),
    ]
}

fn preset(
    id: &str,
    name: &str,
    description: &str,
    role: &str,
    permissions: &[&str],
) -> PermissionPreset {
    PermissionPreset {
        id: id.into(),
        name: name.into(),
        description: description.into(),
        role: role.into(),
        permissions: serde_json::json!({ "allow": permissions }),
    }
}
