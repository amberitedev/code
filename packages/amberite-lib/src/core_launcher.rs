//! Core process management — download, spawn, monitor, and stop the Core binary.

use crate::error::{AmberiteError, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::{path::PathBuf, sync::OnceLock, time::Duration};
use tokio::{process::{Child, Command}, sync::Mutex};

const CORE_BINARY_NAME: &str = if cfg!(windows) {
    "amberite-core.exe"
} else {
    "amberite-core"
};
const DEFAULT_CORE_URL: &str = "http://localhost:16662";

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

static CORE_PROCESS: OnceLock<Mutex<Option<CoreProcess>>> = OnceLock::new();

fn core_process() -> &'static Mutex<Option<CoreProcess>> {
    CORE_PROCESS.get_or_init(|| Mutex::new(None))
}

fn core_install_dir() -> Result<PathBuf> {
    let base = dirs::data_dir().ok_or_else(|| {
        AmberiteError::Core("Cannot find data directory".into())
    })?;
    Ok(base.join("amberite-core"))
}

fn core_data_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("amberite-core")
        .join("data")
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
        return Err(AmberiteError::Core(format!(
            "Download failed: HTTP {}",
            resp.status()
        )));
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
        let mut perms = tokio::fs::metadata(core_binary_path()?)
            .await?
            .permissions();
        perms.set_mode(0o755);
        tokio::fs::set_permissions(core_binary_path()?, perms).await?;
    }
    Ok(())
}

pub async fn install_core_from_url(download_url: &str) -> Result<()> {
    install_core(&Client::new(), download_url).await
}

async fn extract_binary(zip_path: &PathBuf, dest_dir: &PathBuf) -> Result<()> {
    let zip_path = zip_path.clone();
    let dest_dir = dest_dir.clone();
    tokio::task::spawn_blocking(move || {
        let file = std::fs::File::open(&zip_path)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        archive
            .extract(&dest_dir)
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
        return Err(AmberiteError::Core(
            "Core binary not installed. Call install_core first.".into(),
        ));
    }

    let core_data = core_data_dir();

    let child = Command::new(binary)
        .env("AMBERITE_DATA_DIR", &core_data)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    Ok(CoreProcess {
        core_url: DEFAULT_CORE_URL.to_string(),
        child,
    })
}

pub async fn start_managed_core() -> Result<String> {
    let mut process = core_process().lock().await;
    if let Some(existing) = process.as_mut() {
        match existing.child.try_wait() {
            Ok(None) => return Ok(existing.core_url.clone()),
            Ok(Some(_)) => *process = None,
            Err(err) => return Err(AmberiteError::Core(err.to_string())),
        }
    }

    let core = start_core().await?;
    let core_url = core.core_url.clone();
    *process = Some(core);
    Ok(core_url)
}

/// Read the one-time local setup secret emitted by an unpaired app-launched Core.
pub async fn get_local_setup_secret() -> Result<Option<String>> {
    let path = core_data_dir().join(".setup_secret");
    let secret = match tokio::fs::read_to_string(path).await {
        Ok(secret) => secret.trim().to_string(),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(None);
        }
        Err(err) => return Err(AmberiteError::Core(err.to_string())),
    };

    if secret.is_empty() {
        Ok(None)
    } else {
        Ok(Some(secret))
    }
}

/// Check whether the local Core process is responding on its HTTP port.
pub async fn is_core_running(core_url: Option<&str>) -> bool {
    let url = core_url.unwrap_or(DEFAULT_CORE_URL);
    let Ok(client) = Client::builder().timeout(Duration::from_secs(2)).build() else {
        return false;
    };
    match client
        .get(format!("{}/health", url.trim_end_matches('/')))
        .send()
        .await
    {
        Ok(resp) => resp.status().is_success(),
        Err(_) => false,
    }
}

/// Kill a running Core child process.
pub async fn stop_core(process: &mut CoreProcess) -> Result<()> {
    process
        .child
        .kill()
        .await
        .map_err(|e| AmberiteError::Core(e.to_string()))
}

pub async fn stop_managed_core() -> Result<()> {
    let mut process = core_process().lock().await;
    if let Some(mut core) = process.take() {
        stop_core(&mut core).await?;
    }
    Ok(())
}
