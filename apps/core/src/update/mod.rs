//! Updating Copal itself. Today a release is installed by npm or by the curl installer
//! (`scripts/install.sh`), and an update reruns the same one.

#![allow(dead_code)] // Wired in step 6: the self-updater replaces these with signed versions.

pub mod metadata;

use std::path::Path;

use color_eyre::eyre::{Result, bail};
use tokio::process::Command;

use metadata::InstallMethod;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Stable,
    Beta,
    Alpha,
}

impl Channel {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
            Self::Alpha => "alpha",
        }
    }
}

/// Install the latest release on `channel` the way the running one was installed, and return
/// the version now installed.
pub async fn update(channel: Channel) -> Result<String> {
    let Some(installed) = metadata::load()? else {
        bail!(
            "Copal installation metadata is missing. Reinstall with npm or the curl installer."
        )
    };
    match installed.install_method {
        InstallMethod::Npm => npm_update(channel).await?,
        InstallMethod::Curl => curl_update(channel).await?,
        InstallMethod::Manual => bail!(
            "Manual installations cannot be updated automatically. Install with npm or curl first."
        ),
    }
    let version = installed_version(&installed.binary_path).await?;
    metadata::save(
        installed.install_method,
        channel.as_str(),
        installed.data_dir,
        version.clone(),
    )?;
    Ok(version)
}

async fn curl_update(channel: Channel) -> Result<()> {
    let installer = "https://raw.githubusercontent.com/amberitedev/code/main/apps/core/scripts/install.sh";
    let command = format!(
        "curl --fail --location --silent --show-error {installer} | sh -s -- --channel {}",
        channel.as_str()
    );
    let status = Command::new("sh").args(["-c", &command]).status().await?;
    if !status.success() {
        bail!("The Copal curl installer failed with {status}")
    }
    Ok(())
}

async fn npm_update(channel: Channel) -> Result<()> {
    let tag = match channel {
        Channel::Stable => "latest",
        other => other.as_str(),
    };
    let package = format!("@amberitedev/copal@{tag}");
    let status = Command::new("npm")
        .args(["install", "--global", &package])
        .status()
        .await?;
    if !status.success() {
        bail!("npm could not update {package}: {status}")
    }
    Ok(())
}

/// The version the binary at `binary` reports.
async fn installed_version(binary: &Path) -> Result<String> {
    let output = Command::new(binary).arg("--version").output().await?;
    if !output.status.success() {
        bail!("Updated Copal binary could not report its version.")
    }
    let version = String::from_utf8(output.stdout)?
        .trim()
        .strip_prefix("copal ")
        .unwrap_or_default()
        .to_owned();
    if version.is_empty() {
        bail!("Updated Copal binary returned an invalid version.")
    }
    Ok(version)
}
