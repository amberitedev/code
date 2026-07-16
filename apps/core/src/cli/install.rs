use std::path::PathBuf;

use clap::Args;
use color_eyre::eyre::{bail, Result};
use tokio::process::Command;

use super::{
    metadata::{self, InstallMethod},
    require_confirmation, UninstallArgs,
};

#[derive(Debug, Args)]
pub(crate) struct InstallArgs {
    /// Record npm as the installation source used by `copal update`.
    #[arg(long, conflicts_with = "curl")]
    npm: bool,
    /// Record the curl installer as the installation source used by `copal update`.
    #[arg(long)]
    curl: bool,
    /// Do not enable or start the service after installation.
    #[arg(long)]
    no_start: bool,
    /// Environment file containing the required Core configuration.
    #[arg(long)]
    env_file: Option<PathBuf>,
    /// Print the generated systemd unit without changing the system.
    #[arg(long)]
    dry_run: bool,
}

pub(crate) async fn execute(args: InstallArgs) -> Result<()> {
    if !cfg!(target_os = "linux") {
        bail!(
            "System service installation is currently supported on Linux only."
        )
    }
    let method = if args.npm {
        InstallMethod::Npm
    } else if args.curl {
        InstallMethod::Curl
    } else {
        InstallMethod::Manual
    };
    let env_file = match args.env_file {
        Some(path) => path,
        None if std::io::IsTerminal::is_terminal(&std::io::stdin()) => {
            PathBuf::from(
                inquire::Text::new("Path to the Core environment file")
                    .prompt()?,
            )
        }
        None => bail!("--env-file is required in non-interactive mode."),
    };
    if !env_file.is_file() {
        bail!("Environment file does not exist: {}", env_file.display())
    }
    let binary = std::env::current_exe()?;
    let unit = systemd_unit(&binary);
    if args.dry_run {
        println!("{unit}");
        return Ok(());
    }
    let data_dir = data_dir_from_env_file(&env_file).ok_or_else(|| {
        color_eyre::eyre::eyre!(
            "CORE_DATA_DIR is required in the Core environment file."
        )
    })?;
    install_systemd_service(&env_file, &unit, &data_dir, args.no_start).await?;
    metadata::save(method, "stable", Some(data_dir))?;
    println!("Copal service installed successfully.");
    println!("If you were added to the copal group, sign out and back in before using local management commands.");
    Ok(())
}

fn data_dir_from_env_file(path: &std::path::Path) -> Option<PathBuf> {
    std::fs::read_to_string(path)
        .ok()?
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "CORE_DATA_DIR")
                .then(|| PathBuf::from(value.trim().trim_matches('"')))
        })
}

pub(crate) async fn uninstall(args: UninstallArgs) -> Result<()> {
    require_confirmation(args.yes, "Remove Copal service integration?")?;
    if args.purge_data {
        require_confirmation(
            args.yes,
            "Permanently delete Core data as well?",
        )?;
    }
    println!("Run `sudo systemctl disable --now copal.service` and remove the system unit after confirming your Core data backup.");
    println!("Copal intentionally does not remove data or service files without administrator elevation.");
    Ok(())
}

fn systemd_unit(binary: &std::path::Path) -> String {
    let executable = binary
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("[Unit]\nDescription=Copal Core\nAfter=network-online.target\nWants=network-online.target\n\n[Service]\nType=simple\nUser=copal\nGroup=copal\nEnvironmentFile=/etc/copal/copal.env\nExecStart=\"{executable}\" run\nRestart=on-failure\nRestartSec=5\nNoNewPrivileges=true\nPrivateTmp=true\n\n[Install]\nWantedBy=multi-user.target\n")
}

async fn install_systemd_service(
    env_file: &std::path::Path,
    unit: &str,
    data_dir: &std::path::Path,
    no_start: bool,
) -> Result<()> {
    let temporary_unit = std::env::temp_dir().join("copal.service");
    tokio::fs::write(&temporary_unit, unit).await?;
    run_sudo(&["groupadd", "--force", "--system", "copal"]).await?;
    let administrator = std::env::var("SUDO_USER")
        .or_else(|_| std::env::var("USER"))
        .unwrap_or_else(|_| whoami::username());
    if !administrator.is_empty() {
        run_sudo(&["usermod", "--append", "--groups", "copal", &administrator])
            .await?;
    }
    let existing_user = Command::new("id")
        .args(["-u", "copal"])
        .status()
        .await?
        .success();
    if !existing_user {
        run_sudo(&[
            "useradd",
            "--system",
            "--gid",
            "copal",
            "--home-dir",
            "/var/lib/copal",
            "--create-home",
            "--shell",
            "/usr/sbin/nologin",
            "copal",
        ])
        .await?;
    }
    run_sudo(&[
        "install",
        "-d",
        "-o",
        "root",
        "-g",
        "copal",
        "-m",
        "0750",
        "/etc/copal",
    ])
    .await?;
    let data_dir = data_dir.display().to_string();
    run_sudo(&[
        "install", "-d", "-o", "copal", "-g", "copal", "-m", "0750", &data_dir,
    ])
    .await?;
    let env_path = env_file.display().to_string();
    let unit_path = temporary_unit.display().to_string();
    run_sudo(&[
        "install",
        "-o",
        "root",
        "-g",
        "copal",
        "-m",
        "0640",
        &env_path,
        "/etc/copal/copal.env",
    ])
    .await?;
    run_sudo(&[
        "install",
        "-o",
        "root",
        "-g",
        "root",
        "-m",
        "0644",
        &unit_path,
        "/etc/systemd/system/copal.service",
    ])
    .await?;
    run_sudo(&["systemctl", "daemon-reload"]).await?;
    if !no_start {
        run_sudo(&["systemctl", "enable", "--now", "copal.service"]).await?;
    }
    tokio::fs::remove_file(temporary_unit).await.ok();
    Ok(())
}

async fn run_sudo(args: &[&str]) -> Result<()> {
    let status = Command::new("sudo").args(args).status().await?;
    if status.success() {
        Ok(())
    } else {
        bail!("sudo {} failed with {status}", args.join(" "))
    }
}
