use clap::{Args, ValueEnum};
use color_eyre::eyre::{bail, Result};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::process::Command;

use super::metadata::{self, InstallMethod};

#[derive(Debug, Args)]
pub(crate) struct UpdateArgs {
    #[arg(long, value_enum, default_value_t = Channel::Stable)]
    channel: Channel,
    #[arg(long)]
    check: bool,
    #[arg(short = 'y', long)]
    yes: bool,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
enum Channel {
    Stable,
    Beta,
    Alpha,
}

pub(crate) async fn execute(args: UpdateArgs) -> Result<()> {
    let channel = match args.channel {
        Channel::Stable => "stable",
        Channel::Beta => "beta",
        Channel::Alpha => "alpha",
    };
    let Some(metadata) = metadata::load()? else {
        bail!("Copal installation metadata is missing. Re-run `copal install --npm` or `copal install --curl`.")
    };
    if args.check {
        println!(
            "Installed version: {}\nSelected update channel: {channel}",
            metadata.installed_version
        );
        return Ok(());
    }
    let progress = ProgressBar::new_spinner();
    progress.set_style(ProgressStyle::with_template("{spinner} {msg}")?);
    progress.enable_steady_tick(std::time::Duration::from_millis(80));
    progress.set_message(format!("Updating Copal from the {channel} channel"));
    let result = match &metadata.install_method {
		InstallMethod::Npm => npm_update(channel).await,
		InstallMethod::Curl => curl_update(channel).await,
		InstallMethod::Manual => bail!("Manual installations cannot be updated automatically. Install with npm or curl first."),
	};
    progress.finish_and_clear();
    result?;
    let version = installed_version(&metadata.binary_path).await?;
    metadata::save_with_version(
        metadata.install_method,
        channel,
        metadata.data_dir,
        version,
    )?;
    if super::service::restart_after_update(args.yes).await? {
        println!("Copal update completed and the Core service was restarted successfully.");
    } else {
        println!("Copal update completed. The Core service was not running, so it was left stopped.");
    }
    Ok(())
}

async fn curl_update(channel: &str) -> Result<()> {
    let installer = "https://raw.githubusercontent.com/amberitedev/code/main/apps/core/scripts/install.sh";
    let command = format!("curl --fail --location --silent --show-error {installer} | sh -s -- --channel {channel}");
    let status = Command::new("sh").args(["-c", &command]).status().await?;
    if status.success() {
        Ok(())
    } else {
        bail!("The Copal curl installer failed with {status}")
    }
}

async fn installed_version(binary: &std::path::Path) -> Result<String> {
    let output = Command::new(binary).arg("version").output().await?;
    if !output.status.success() {
        bail!("Updated Copal binary could not report its version.")
    }
    let version = String::from_utf8(output.stdout)?
        .trim()
        .strip_prefix("copal ")
        .unwrap_or("")
        .to_owned();
    if version.is_empty() {
        bail!("Updated Copal binary returned an invalid version.")
    }
    Ok(version)
}

async fn npm_update(channel: &str) -> Result<()> {
    let tag = match channel {
        "stable" => "latest",
        other => other,
    };
    let package = format!("@amberitedev/copal@{tag}");
    let status = Command::new("npm")
        .args(["install", "--global", &package])
        .status()
        .await?;
    if status.success() {
        Ok(())
    } else {
        bail!("npm could not update {package}: {status}")
    }
}
