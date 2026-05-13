//! Core process management — download, spawn, monitor, and stop the Core binary.

use crate::error::{AmberiteError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::{Child, Command};

const CORE_BINARY_NAME: &str = if cfg!(windows) { "amberite-core.exe" } else { "amberite-core" };
const DEFAULT_CORE_URL: &str = "http://localhost:7000";

/// Status of the local Core process.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CoreStatus {
    Running,
    Stopped,
}

/// Handle to a running Core process.
pub struct CoreProcess {
    pub core_url: String,
    child: Child,
}

fn core_install_dir() -> Result<PathBuf> {
    let base = dirs::data_dir()
        .ok_or_else(|| AmberiteError::Core("Cannot find data directory".into()))?;
    Ok(base.join("amberite-core"))
}

fn core_binary_path() -> Result<PathBuf> {
    Ok(core_install_dir()?.join(CORE_BINARY_NAME))
}

/// Returns whether the Core binary exists on disk.
pub fn is_core_installed() -> bool {
    core_binary_path().map(|p| p.exists()).unwrap_or(false)
}

/// Download the Core binary from a provided URL and extract it.
pub async fn install_core(client: &Client, download_url: &str) -> Result<()> {
    let resp = client.get(download_url).send().await?;
    if !resp.status().is_success() {
        return Err(AmberiteError::Core(format!("Download failed: HTTP {}", resp.status())));
    }
    let bytes = resp.bytes().await?;
    let install_dir = core_install_dir()?;
    tokio::fs::create_dir_all(&install_dir).await?;

    let zip_path = install_dir.join("core_download.zip");
    tokio::fs::write(&zip_path, &bytes).await?;
    extract_binary(&zip_path, &install_dir).await?;
    tokio::fs::remove_file(&zip_path).await.ok();

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = tokio::fs::metadata(core_binary_path()?).await?.permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(core_binary_path()?, perms).await?;
    }
    Ok(())
}

async fn extract_binary(zip_path: &PathBuf, dest_dir: &PathBuf) -> Result<()> {
    let zip_path = zip_path.clone();
    let dest_dir = dest_dir.clone();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        archive.extract(&dest_dir)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        Ok::<_, std::io::Error>(())
    })
    .await
    .map_err(|e| AmberiteError::Core(e.to_string()))??;
    Ok(())
}

/// Spawn the Core binary as a background child process.
pub async fn start_core() -> Result<CoreProcess> {
    let binary = core_binary_path()?;
    if !binary.exists() {
        return Err(AmberiteError::Core("Core binary not installed. Call install_core first.".into()));
    }

    let core_data = dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("amberite-core")
        .join("data");

    let child = Command::new(binary)
        .arg("--data-dir")
        .arg(&core_data)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    Ok(CoreProcess { core_url: DEFAULT_CORE_URL.to_string(), child })
}

/// Check whether the local Core process is responding on its HTTP port.
pub async fn is_core_running(core_url: Option<&str>) -> bool {
    let url = core_url.unwrap_or(DEFAULT_CORE_URL);
    match Client::new().get(format!("{}/health", url.trim_end_matches('/'))).send().await {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Read the local Core token from the well-known file path.
pub async fn get_local_core_token() -> Result<String> {
    let token_file = dirs::data_dir()
        .ok_or_else(|| AmberiteError::Core("Cannot find data directory".into()))?
        .join("amberite-core")
        .join("data")
        .join(".local_token");

    if !token_file.exists() {
        return Err(AmberiteError::Core("Local Core token file not found".into()));
    }

    tokio::fs::read_to_string(&token_file)
        .await
        .map(|s| s.trim().to_string())
        .map_err(|e| AmberiteError::Core(format!("Failed to read local Core token: {e}")))
}

/// Kill a running Core child process.
pub async fn stop_core(process: &mut CoreProcess) -> Result<()> {
    process.child.kill().await.map_err(|e| AmberiteError::Core(e.to_string()))
}
