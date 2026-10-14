//! How this copy of Copal was installed, kept next to the user's other local data.

use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use color_eyre::eyre::{OptionExt, Result, WrapErr};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallMetadata {
    pub install_method: InstallMethod,
    pub channel: String,
    pub installed_version: String,
    pub binary_path: PathBuf,
    pub data_dir: Option<PathBuf>,
    pub updated_at_unix: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallMethod {
    Npm,
    Curl,
    Manual,
}

pub fn path() -> Result<PathBuf> {
    let home = std::env::home_dir()
        .ok_or_eyre("Unable to determine the current user's home directory")?;
    Ok(home.join(".local/share/copal/install.json"))
}

pub fn load() -> Result<Option<InstallMetadata>> {
    let path = path()?;
    if !path.exists() {
        return Ok(None);
    }
    let content = std::fs::read_to_string(&path)
        .wrap_err_with(|| format!("Unable to read {}", path.display()))?;
    Ok(Some(
        serde_json::from_str(&content)
            .wrap_err("Copal installation metadata is invalid")?,
    ))
}

pub fn save(
    install_method: InstallMethod,
    channel: &str,
    data_dir: Option<PathBuf>,
    installed_version: String,
) -> Result<()> {
    let path = path()?;
    std::fs::create_dir_all(path.parent().expect("metadata path has parent"))?;
    let metadata = InstallMetadata {
        install_method,
        channel: channel.to_owned(),
        installed_version,
        binary_path: std::env::current_exe()?,
        data_dir,
        updated_at_unix: SystemTime::now()
            .duration_since(UNIX_EPOCH)?
            .as_secs(),
    };
    std::fs::write(
        &path,
        format!("{}\n", serde_json::to_string_pretty(&metadata)?),
    )?;
    Ok(())
}
