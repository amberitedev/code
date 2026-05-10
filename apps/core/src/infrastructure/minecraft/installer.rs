use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tokio::io::AsyncBufReadExt;
use tracing::{info, warn};

use crate::domain::instance::ModLoader;

#[derive(Debug, thiserror::Error)]
pub enum InstallerError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unsupported mc version for this loader: {0}")]
    UnsupportedVersion(String),
    #[error("installer failed with code {0}")]
    InstallerFailed(i32),
    #[error("xml parse error: {0}")]
    XmlParse(String),
}

/// How the server is launched after installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LaunchStyle {
    /// Plain `-jar <jar>` invocation.
    Jar { jar: String },
    /// Forge/NeoForge args-file (`@libraries/...args.txt`).
    ArgsFile { args: String },
}

/// Persisted to `{data_dir}/launch.json` after installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LaunchConfig {
    pub style: LaunchStyle,
}

pub async fn write_launch_config(data_dir: &Path, config: &LaunchConfig) -> Result<(), InstallerError> {
    let path = data_dir.join("launch.json");
    let json = serde_json::to_string_pretty(config)
        .map_err(|e| InstallerError::Io(std::io::Error::other(e.to_string())))?;
    tokio::fs::write(&path, json).await?;
    Ok(())
}

pub async fn read_launch_config(data_dir: &Path) -> Option<LaunchConfig> {
    let path = data_dir.join("launch.json");
    let bytes = tokio::fs::read(&path).await.ok()?;
    serde_json::from_slice(&bytes).ok()
}

pub async fn install_with_installer(
    http: &reqwest::Client,
    loader: &ModLoader,
    mc_version: &str,
    loader_version: Option<&str>,
    data_dir: &Path,
    java_path: &Path,
) -> Result<(), InstallerError> {
    match loader {
        ModLoader::Quilt => install_quilt(http, mc_version, loader_version, data_dir, java_path).await,
        ModLoader::Forge => install_forge(http, mc_version, loader_version, data_dir, java_path).await,
        ModLoader::NeoForge => install_neoforge(http, mc_version, loader_version, data_dir, java_path).await,
        _ => Err(InstallerError::UnsupportedVersion(format!("{loader:?} does not use installer"))),
    }
}

async fn install_quilt(
    http: &reqwest::Client,
    mc_version: &str,
    _loader_version: Option<&str>,
    data_dir: &Path,
    java_path: &Path,
) -> Result<(), InstallerError> {
    let version = fetch_maven_release(
        http,
        "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer/maven-metadata.xml",
    ).await?;
    let url = format!(
        "https://maven.quiltmc.org/repository/release/org/quiltmc/quilt-installer/{version}/quilt-installer-{version}.jar"
    );
    let installer = download_to_temp(http, &url, "quilt-installer.jar").await?;
    run_installer(java_path, installer.path(), &[
        "install", "server", mc_version, "--install-dir", data_dir.to_str().unwrap_or("."),
    ]).await?;
    write_launch_config(data_dir, &LaunchConfig {
        style: LaunchStyle::Jar { jar: "quilt-server-launch.jar".into() },
    }).await
}

async fn install_forge(
    http: &reqwest::Client,
    mc_version: &str,
    loader_version: Option<&str>,
    data_dir: &Path,
    java_path: &Path,
) -> Result<(), InstallerError> {
    let forge_version = match loader_version {
        Some(v) => v.to_string(),
        None => fetch_forge_version(http, mc_version).await?,
    };
    let url = format!(
        "https://files.minecraftforge.net/net/minecraftforge/forge/{mc_version}-{forge_version}/forge-{mc_version}-{forge_version}-installer.jar"
    );
    let installer = download_to_temp(http, &url, "forge-installer.jar").await?;
    run_installer(java_path, installer.path(), &[
        "--installServer", data_dir.to_str().unwrap_or("."),
    ]).await?;
    let style = detect_launch_style(data_dir).await;
    write_launch_config(data_dir, &LaunchConfig { style }).await
}

async fn install_neoforge(
    http: &reqwest::Client,
    mc_version: &str,
    _loader_version: Option<&str>,
    data_dir: &Path,
    java_path: &Path,
) -> Result<(), InstallerError> {
    // NeoForge requires MC 1.20.1+
    let parts: Vec<u32> = mc_version.split('.').filter_map(|p| p.parse().ok()).collect();
    let (major, minor) = (parts.get(1).copied().unwrap_or(0), parts.get(2).copied().unwrap_or(0));
    if major < 20 || (major == 20 && minor < 1) {
        return Err(InstallerError::UnsupportedVersion(
            format!("NeoForge requires MC 1.20.1+; got {mc_version}")
        ));
    }
    let nf_version = fetch_maven_release(
        http,
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/maven-metadata.xml",
    ).await?;
    let url = format!(
        "https://maven.neoforged.net/releases/net/neoforged/neoforge/{nf_version}/neoforge-{nf_version}-installer.jar"
    );
    let installer = download_to_temp(http, &url, "neoforge-installer.jar").await?;
    run_installer(java_path, installer.path(), &[
        "--installServer", data_dir.to_str().unwrap_or("."),
    ]).await?;
    let style = detect_launch_style(data_dir).await;
    write_launch_config(data_dir, &LaunchConfig { style }).await
}

async fn fetch_maven_release(http: &reqwest::Client, url: &str) -> Result<String, InstallerError> {
    let xml = http.get(url).send().await?.error_for_status()?.text().await?;
    // Extract <release>...</release> tag
    xml.find("<release>")
        .and_then(|s| {
            let after = &xml[s + "<release>".len()..];
            after.find("</release>").map(|e| after[..e].trim().to_string())
        })
        .ok_or_else(|| InstallerError::XmlParse(format!("no <release> in {url}")))
}

async fn fetch_forge_version(http: &reqwest::Client, mc_version: &str) -> Result<String, InstallerError> {
    #[derive(Deserialize)]
    struct Promos { promos: std::collections::HashMap<String, String> }
    let promos: Promos = http
        .get("https://files.minecraftforge.net/net/minecraftforge/forge/promotions_slim.json")
        .send().await?.error_for_status()?.json().await?;
    let key_rec = format!("{mc_version}-recommended");
    let key_lat = format!("{mc_version}-latest");
    promos.promos.get(&key_rec)
        .or_else(|| promos.promos.get(&key_lat))
        .cloned()
        .ok_or_else(|| InstallerError::UnsupportedVersion(format!("no Forge build for {mc_version}")))
}

async fn download_to_temp(http: &reqwest::Client, url: &str, name: &str) -> Result<tempfile::NamedTempFile, InstallerError> {
    info!("Downloading installer from {url}");
    let bytes = http.get(url).send().await?.error_for_status()?.bytes().await?;
    let suffix = format!("-{name}");
    let tmp = tempfile::Builder::new().suffix(&suffix).tempfile()
        .map_err(InstallerError::Io)?;
    tokio::fs::write(tmp.path(), &bytes).await?;
    Ok(tmp)
}

async fn run_installer(java_path: &Path, jar: &Path, args: &[&str]) -> Result<(), InstallerError> {
    let mut cmd_args = vec!["-jar", jar.to_str().unwrap_or("installer.jar")];
    cmd_args.extend_from_slice(args);
    let mut child = tokio::process::Command::new(java_path)
        .args(&cmd_args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    // Drain stdout/stderr to avoid pipe buffer deadlock
    if let Some(stdout) = child.stdout.take() {
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        tokio::spawn(async move {
            while let Ok(Some(l)) = lines.next_line().await {
                info!("[installer] {l}");
            }
        });
    }
    if let Some(stderr) = child.stderr.take() {
        let mut lines = tokio::io::BufReader::new(stderr).lines();
        tokio::spawn(async move {
            while let Ok(Some(l)) = lines.next_line().await {
                warn!("[installer:err] {l}");
            }
        });
    }
    let status = child.wait().await?;
    if !status.success() {
        return Err(InstallerError::InstallerFailed(status.code().unwrap_or(-1)));
    }
    Ok(())
}

/// Detect Forge/NeoForge launch style by checking for run.sh or run.bat.
async fn detect_launch_style(data_dir: &Path) -> LaunchStyle {
    for script in &["run.sh", "run.bat"] {
        let path = data_dir.join(script);
        if let Ok(content) = tokio::fs::read_to_string(&path).await {
            // Forge 1.17+ uses @libraries/...args.txt reference
            if let Some(pos) = content.find("@libraries") {
                let rest = &content[pos + 1..]; // skip '@'
                let end = rest.find(|c: char| c.is_whitespace()).unwrap_or(rest.len());
                let args_path: PathBuf = rest[..end].into();
                return LaunchStyle::ArgsFile {
                    args: args_path.display().to_string(),
                };
            }
        }
    }
    // Fallback: plain jar (Forge < 1.17)
    LaunchStyle::Jar { jar: "forge-server.jar".into() }
}
