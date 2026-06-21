use std::{
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

use color_eyre::eyre::{Result, WrapErr};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct InstallMetadata {
    pub(crate) install_method: InstallMethod,
    pub(crate) channel: String,
    pub(crate) installed_version: String,
    pub(crate) binary_path: PathBuf,
    pub(crate) data_dir: Option<PathBuf>,
    pub(crate) updated_at_unix: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InstallMethod {
    Npm,
    Curl,
    Manual,
}

pub(crate) fn path() -> Result<PathBuf> {
    let home = home::home_dir().ok_or_else(|| {
        color_eyre::eyre::eyre!(
            "Unable to determine the current user's home directory"
        )
    })?;
    Ok(home.join(".local/share/copal/install.json"))
}

pub(crate) fn load() -> Result<Option<InstallMetadata>> {
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

pub(crate) fn save(
    install_method: InstallMethod,
    channel: &str,
    data_dir: Option<PathBuf>,
) -> Result<()> {
    save_with_version(
        install_method,
        channel,
        data_dir,
        env!("CARGO_PKG_VERSION").to_owned(),
    )
}

pub(crate) fn save_with_version(
    install_method: InstallMethod,
    channel: &str,
    data_dir: Option<PathBuf>,
    installed_version: String,
) -> Result<()> {
    let path = path()?;
    let parent = path.parent().expect("metadata path has parent");
    std::fs::create_dir_all(parent)?;
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
