use std::path::{Path, PathBuf};

use sha1::Digest;
use tracing::info;

use super::{
    flavours::{resolve_jar, FlavourError},
    installer::{
        install_with_installer, write_launch_config, InstallerError,
        LaunchConfig, LaunchStyle,
    },
    shared::{self, SharedError},
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
    #[error("shared store: {0}")]
    Shared(#[from] SharedError),
    #[error("sha1 mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
}

/// Install the server for a build and write a `launch.json` to `installation_dir`.
///
/// Routing by loader:
/// * Vanilla / Fabric / Quilt → the global shared store (`{data_dir}/meta`),
///   writing `LaunchStyle::Modular` with absolute launch tokens.
/// * Forge / NeoForge → the vendor installer, which configures the
///   installation dir and writes a `Jar`/`ArgsFile` launch style.
/// * Paper → a single URL-based jar written into the installation dir.
///
/// `data_dir` is the Core data root used to locate the shared store; it differs
/// from `installation_dir` (`{data_dir}/installations/{id}`), which only holds
/// the per-build `launch.json`. For installer-based loaders the Java process is
/// invoked, so `java_path` should be provided (falls back to `"java"`).
pub async fn download_server_jar(
    http: &reqwest::Client,
    loader: &ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
    installation_dir: &Path,
    data_dir: &Path,
    java_path: Option<&Path>,
) -> Result<PathBuf, ServerJarError> {
    let java = java_path.unwrap_or_else(|| Path::new("java"));
    let java_arch = std::env::consts::ARCH;

    match loader {
        ModLoader::Vanilla | ModLoader::Fabric | ModLoader::Quilt => {
            let args = shared::install_shared(
                http,
                data_dir,
                loader,
                game_version,
                loader_version,
                java_path,
                java_arch,
            )
            .await?;
            write_launch_config(
                installation_dir,
                &LaunchConfig {
                    style: LaunchStyle::Modular { args },
                },
            )
            .await?;
            Ok(installation_dir.join("launch.json"))
        }
        ModLoader::Forge | ModLoader::NeoForge => {
            // Installer-based: let the installer download and configure everything.
            install_with_installer(
                http,
                loader,
                game_version,
                loader_version,
                installation_dir,
                java,
            )
            .await?;
            Ok(installation_dir.join("server.jar")) // path is nominal; actual jar set in launch.json
        }
        ModLoader::Paper => {
            let info =
                resolve_jar(http, loader, game_version, loader_version).await?;
            let dest = installation_dir.join(&info.filename);
            info!("Downloading server JAR from {}", info.url);
            let bytes = http
                .get(&info.url)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            if let Some(expected) = info.sha1 {
                let actual = hex::encode(sha1::Sha1::digest(&bytes));
                if actual != expected {
                    return Err(ServerJarError::HashMismatch {
                        expected,
                        actual,
                    });
                }
            }
            tokio::fs::write(&dest, &bytes).await?;
            info!("Server JAR written to {}", dest.display());
            // Write launch.json so instance_status_service knows how to start this jar.
            write_launch_config(
                installation_dir,
                &LaunchConfig {
                    style: LaunchStyle::Jar { jar: info.filename },
                },
            )
            .await?;
            Ok(dest)
        }
    }
}
