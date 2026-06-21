use std::process::Stdio;

use clap::{Args, Subcommand};
use color_eyre::eyre::{bail, Result};
use serde::Serialize;
use tokio::process::Command;

use super::{
    admin::{self, Request, Response},
    metadata, OutputFormat,
};

#[derive(Debug, Args)]
pub(crate) struct ServiceArgs {
    #[command(subcommand)]
    command: ServiceCommand,
}

#[derive(Debug, Subcommand)]
enum ServiceCommand {
    Status,
    Start,
    Stop {
        #[arg(short = 'f', long)]
        force: bool,
        #[arg(long)]
        stop_instances: bool,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    Restart {
        #[arg(short = 'f', long)]
        force: bool,
        #[arg(long)]
        stop_instances: bool,
        #[arg(short = 'y', long)]
        yes: bool,
    },
    Logs {
        #[arg(short = 'n', long, default_value_t = 100)]
        lines: u16,
        #[arg(short = 'f', long)]
        follow: bool,
    },
    Enable,
    Disable,
}

#[derive(Serialize)]
struct ServiceStatus {
    manager: &'static str,
    installed: bool,
    active: bool,
}

pub(crate) async fn execute(
    args: ServiceArgs,
    format: OutputFormat,
) -> Result<()> {
    match args.command {
        ServiceCommand::Status => status(format).await,
        ServiceCommand::Start => systemctl(&["start", "copal.service"]).await,
        ServiceCommand::Stop {
            force,
            stop_instances,
            yes,
        } => {
            prepare_shutdown(stop_instances, force, yes).await?;
            if force {
                systemctl(&["kill", "copal.service"]).await
            } else {
                systemctl(&["stop", "copal.service"]).await
            }
        }
        ServiceCommand::Restart {
            force,
            stop_instances,
            yes,
        } => {
            prepare_shutdown(stop_instances, force, yes).await?;
            if force {
                systemctl(&["kill", "copal.service"]).await?;
            }
            systemctl(&["restart", "copal.service"]).await
        }
        ServiceCommand::Logs { lines, follow } => {
            let mut command = Command::new("journalctl");
            command.args([
                "-u",
                "copal.service",
                "-n",
                &lines.to_string(),
                "--no-pager",
            ]);
            if follow {
                command.arg("-f");
            }
            let status = command.status().await?;
            if status.success() {
                Ok(())
            } else {
                bail!("journalctl failed with {status}")
            }
        }
        ServiceCommand::Enable => systemctl(&["enable", "copal.service"]).await,
        ServiceCommand::Disable => {
            systemctl(&["disable", "copal.service"]).await
        }
    }
}

pub(crate) async fn menu() -> Result<()> {
    let choice = inquire::Select::new(
        "Service controls",
        vec!["Status", "Start", "Stop", "Restart", "View logs", "Back"],
    )
    .prompt()?;
    match choice {
        "Status" => status(OutputFormat::Table).await,
        "Start" => systemctl(&["start", "copal.service"]).await,
        "Stop" => {
            prepare_shutdown(false, false, false).await?;
            systemctl(&["stop", "copal.service"]).await
        }
        "Restart" => {
            prepare_shutdown(false, false, false).await?;
            systemctl(&["restart", "copal.service"]).await
        }
        "View logs" => {
            let status = Command::new("journalctl")
                .args(["-u", "copal.service", "-n", "100", "--no-pager"])
                .status()
                .await?;
            if status.success() {
                Ok(())
            } else {
                bail!("journalctl failed with {status}")
            }
        }
        _ => Ok(()),
    }
}

async fn prepare_shutdown(
    stop_instances: bool,
    force: bool,
    yes: bool,
) -> Result<()> {
    let Some(metadata) = metadata::load()? else {
        return Ok(());
    };
    let Some(data_dir) = metadata.data_dir else {
        return Ok(());
    };
    let response = match admin::request(&data_dir, Request::ListInstances).await
    {
        Ok(response) => response,
        Err(_) => return Ok(()),
    };
    let Response::Instances { instances } = response else {
        return Ok(());
    };
    let active: Vec<_> = instances
        .into_iter()
        .filter(|instance| {
            matches!(
                instance.status,
                crate::domain::instance::InstanceStatus::Running
                    | crate::domain::instance::InstanceStatus::Starting
                    | crate::domain::instance::InstanceStatus::Stopping
            )
        })
        .collect();
    if active.is_empty() || force {
        return Ok(());
    }
    let stop = if stop_instances {
        true
    } else if !std::io::IsTerminal::is_terminal(&std::io::stdin()) {
        bail!("{} instance(s) are active. Re-run with --stop-instances --yes, or use --force --yes.", active.len())
    } else {
        let choice = inquire::Select::new(
            "Active instances would become unmanaged",
            vec!["Gracefully stop all instances", "Cancel"],
        )
        .prompt()?;
        choice == "Gracefully stop all instances"
    };
    if !stop {
        bail!("Operation cancelled.")
    }
    if !yes && !stop_instances {
        super::require_confirmation(
            false,
            &format!(
                "Stop {} active instance(s) before stopping Core?",
                active.len()
            ),
        )?;
    }
    for instance in active {
        admin::request(
            &data_dir,
            Request::StopInstance {
                id: instance.id.to_string(),
            },
        )
        .await?;
    }
    wait_for_instances(&data_dir).await
}

async fn wait_for_instances(data_dir: &std::path::Path) -> Result<()> {
    let deadline =
        tokio::time::Instant::now() + std::time::Duration::from_secs(45);
    loop {
        let response = admin::request(data_dir, Request::ListInstances).await?;
        let Response::Instances { instances } = response else {
            return Ok(());
        };
        let active = instances.iter().any(|instance| {
            matches!(
                instance.status,
                crate::domain::instance::InstanceStatus::Running
                    | crate::domain::instance::InstanceStatus::Starting
                    | crate::domain::instance::InstanceStatus::Stopping
            )
        });
        if !active {
            return Ok(());
        }
        if tokio::time::Instant::now() >= deadline {
            bail!("Timed out waiting for instances to stop. Use --force only if you accept a hard shutdown.")
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
}

pub(crate) async fn status(format: OutputFormat) -> Result<()> {
    let installed = Command::new("systemctl")
        .args(["cat", "copal.service"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .await
        .map(|s| s.success())
        .unwrap_or(false);
    let active = installed
        && Command::new("systemctl")
            .args(["is-active", "--quiet", "copal.service"])
            .status()
            .await
            .map(|s| s.success())
            .unwrap_or(false);
    let value = ServiceStatus {
        manager: "systemd",
        installed,
        active,
    };
    match format {
        OutputFormat::Json => println!("{}", serde_json::to_string(&value)?),
        OutputFormat::Table => println!(
            "Core service: {}\nStatus: {}",
            if installed {
                "installed"
            } else {
                "not installed"
            },
            if active { "running" } else { "stopped" }
        ),
    }
    Ok(())
}

pub(crate) async fn diagnose(format: OutputFormat) -> Result<()> {
    status(format).await?;
    match super::metadata::load()? {
        Some(metadata) => println!(
            "Install method: {:?}\nInstalled version: {}\nChannel: {}",
            metadata.install_method,
            metadata.installed_version,
            metadata.channel
        ),
        None => println!("Install metadata: not found"),
    }
    Ok(())
}

pub(crate) async fn restart_after_update(yes: bool) -> Result<bool> {
    let active = Command::new("systemctl")
        .args(["is-active", "--quiet", "copal.service"])
        .status()
        .await
        .map(|status| status.success())
        .unwrap_or(false);
    if !active {
        return Ok(false);
    }
    prepare_shutdown(false, false, yes).await?;
    systemctl(&["restart", "copal.service"]).await?;
    Ok(true)
}

async fn systemctl(args: &[&str]) -> Result<()> {
    let status = Command::new("systemctl").args(args).status().await?;
    if status.success() {
        Ok(())
    } else {
        bail!("systemctl {} failed with {status}", args.join(" "))
    }
}
