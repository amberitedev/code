use std::path::{Path, PathBuf};

use sha1::Digest;
use tracing::info;

use super::{
    flavours::{resolve_jar, FlavourError},
    installer::{install_with_installer, write_launch_config, InstallerError, LaunchConfig, LaunchStyle},
};
use crate::domain::instance::ModLoader;

#[derive(Debug, thiserror::Error)]
pub enum ServerJarError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("flavour: {0}")]
    Flavour(#[from] FlavourError),
    #[error("installer: {0}")]
    Installer(#[from] InstallerError),
    #[error("sha1 mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
}

/// Download/install the server and write a `launch.json` to `instance_dir`.
/// For installer-based loaders (Quilt, Forge, NeoForge) the Java process is invoked;
/// `java_path` is required (falls back to "java" if None).
pub async fn download_server_jar(
    http: &reqwest::Client,
    loader: &ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
    instance_dir: &Path,
    java_path: Option<&Path>,
) -> Result<PathBuf, ServerJarError> {
    let java = java_path.unwrap_or_else(|| Path::new("java"));

    match loader {
        ModLoader::Quilt | ModLoader::Forge | ModLoader::NeoForge => {
            // Installer-based: let the installer download and configure everything.
            install_with_installer(http, loader, game_version, loader_version, instance_dir, java)
                .await?;
            Ok(instance_dir.join("server.jar")) // path is nominal; actual jar set in launch.json
        }
        _ => {
            let info = resolve_jar(http, loader, game_version, loader_version).await?;
            let dest = instance_dir.join(&info.filename);
            info!("Downloading server JAR from {}", info.url);
            let bytes = http.get(&info.url).send().await?.error_for_status()?.bytes().await?;
            if let Some(expected) = info.sha1 {
                let actual = hex::encode(sha1::Sha1::digest(&bytes));
                if actual != expected {
                    return Err(ServerJarError::HashMismatch { expected, actual });
                }
            }
            tokio::fs::write(&dest, &bytes).await?;
            info!("Server JAR written to {}", dest.display());
            // Write launch.json so instance_status_service knows how to start this jar.
            write_launch_config(instance_dir, &LaunchConfig {
                style: LaunchStyle::Jar { jar: info.filename },
            }).await?;
            Ok(dest)
        }
    }
}
