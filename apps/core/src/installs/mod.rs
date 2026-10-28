//! Server installs: the shared store for Vanilla, Fabric, Quilt, and Forge and NeoForge from
//! launcher-meta, the vendor installer for Forge before 1.17, Paper's jar, and the Java runtimes
//! Core manages. Everything shared lives in `meta/` and `java/`, which servers may read but not
//! write.

pub mod download;
pub mod forge;
pub mod java;
pub mod paper;
pub mod shared;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::servers::{Hold, ServerError, Servers, launch, records::ServerRow};
use shared::SharedDirs;

/// The server software a server runs, as `servers.loader` stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Paper,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
}

impl ModLoader {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Vanilla => "vanilla",
            Self::Paper => "paper",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Quilt => "quilt",
        }
    }

    /// From `servers.loader`, or a loader name in any case.
    pub fn parse(name: &str) -> Option<Self> {
        [
            Self::Vanilla,
            Self::Paper,
            Self::Fabric,
            Self::Forge,
            Self::NeoForge,
            Self::Quilt,
        ]
        .into_iter()
        .find(|loader| loader.as_str().eq_ignore_ascii_case(name))
    }
}

impl std::fmt::Display for ModLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Whether `version` is plainly a version: letters, digits, dots, and dashes, starting with a
/// letter or digit, with no `..`. Game and loader versions go into vendor URLs and store paths,
/// so a value like `47.2.0?x=/../../pwn` must never reach either.
pub fn plain_version(version: &str) -> bool {
    !version.is_empty()
        && version.len() <= 64
        && version.starts_with(|c: char| c.is_ascii_alphanumeric())
        && !version.contains("..")
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("{0:?} is not a version")]
    BadVersion(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Java(#[from] java::JavaError),
    #[error(transparent)]
    Paper(#[from] paper::PaperError),
    #[error(transparent)]
    Installer(#[from] forge::InstallerError),
    #[error(transparent)]
    Shared(#[from] shared::SharedError),
    #[error(transparent)]
    Server(#[from] ServerError),
    #[error(transparent)]
    Other(#[from] color_eyre::Report),
}

/// Install `loader` for `game_version` on a held server, with the Java it needs, and return the
/// loader version installed (`None` for Vanilla, the build for Paper). `requested` is a loader
/// version, `latest`, `stable`, or `None` for the newest. `verify` (repair) re-checks every file
/// already there instead of trusting it. Nothing in the shared store is deleted, so other servers
/// keep running. `progress` is told the share done.
pub async fn install(
    servers: &Servers,
    hold: &Hold,
    http: &reqwest::Client,
    server: &ServerRow,
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
    verify: bool,
    progress: &(dyn Fn(f64) + Sync),
) -> Result<Option<String>, InstallError> {
    if let Some(bad) = std::iter::once(game_version)
        .chain(requested)
        .find(|version| !plain_version(version))
    {
        return Err(InstallError::BadVersion(bad.to_owned()));
    }
    let paths = servers.paths();
    let dirs = SharedDirs::new(paths.root());
    let target = ServerRow {
        loader: Some(loader.as_str().into()),
        game_version: Some(game_version.into()),
        ..server.clone()
    };
    progress(0.0);
    // Held until the install is done, so the patch check keeps the Java the installer runs.
    let (java, _lease) = launch::java_for(paths, http, &target).await?;
    progress(0.2);
    let rest = |share: f64| progress(0.2 + share * 0.8);
    let installed = match loader {
        ModLoader::Vanilla | ModLoader::Fabric | ModLoader::Quilt => {
            shared::install_shared(
                http,
                &dirs,
                loader,
                game_version,
                requested,
                verify,
                &rest,
            )
            .await?
        }
        ModLoader::Paper => {
            let received = std::sync::atomic::AtomicU64::new(0);
            let build = paper::install(
                http,
                &dirs,
                game_version,
                requested,
                verify,
                &|update| {
                    if let download::Progress::Bytes {
                        bytes,
                        total: Some(total),
                    } = update
                    {
                        let done = received.fetch_add(
                            bytes,
                            std::sync::atomic::Ordering::Relaxed,
                        ) + bytes;
                        rest(done as f64 / total.max(1) as f64);
                    }
                },
            )
            .await?;
            Some(build)
        }
        ModLoader::Forge | ModLoader::NeoForge => {
            let version =
                forge::resolve(http, loader, game_version, requested).await?;
            let installer = forge::installer(
                http,
                &dirs,
                loader,
                game_version,
                &version.id,
            )
            .await?;
            rest(0.1);
            let dir = paths.server(hold.id());
            if let Some(args) = forge::server_args(&installer).await? {
                let work = dir.join("tmp").join(WORK_FOLDER);
                crate::daemon::folders::create_server_folder(&dir)?;
                crate::daemon::folders::create_server_folder(&dir.join("tmp"))?;
                crate::daemon::folders::create_server_folder(&work)?;
                let installed = forge::install_from_meta(
                    servers,
                    hold,
                    http,
                    &dirs,
                    loader,
                    game_version,
                    &version,
                    &installer,
                    &args,
                    &java,
                    &work,
                    verify,
                    &|share| rest(0.1 + share * 0.9),
                )
                .await;
                remove_installer_leftovers(&dir, None).await;
                installed?;
            } else {
                run_vendor_installer(servers, hold, &installer, java).await?;
            }
            forge::launch_args(&dirs, &dir, loader, game_version, &version.id)?;
            Some(version.id)
        }
    };
    progress(1.0);
    Ok(installed)
}

/// Run a Forge or NeoForge installer in the server folder as `copal-servers`. The installer
/// writes its log beside itself, so it runs from a copy in the server folder, removed after.
async fn run_vendor_installer(
    servers: &Servers,
    hold: &Hold,
    installer: &Path,
    java: std::path::PathBuf,
) -> Result<(), InstallError> {
    let dir = servers.paths().server(hold.id());
    let name = installer
        .file_name()
        .expect("installers have a file name")
        .to_owned();
    let copy = dir.join(&name);
    let bytes = tokio::fs::read(installer).await?;
    launch::write_server_file(&copy, &bytes)?;
    let result = servers
        .run_installer(
            hold,
            java,
            forge::installer_args(Path::new(&name), &dir),
        )
        .await;
    let _ = std::fs::remove_file(&copy);
    result?;
    // Kept when it fails: its log says why.
    remove_installer_leftovers(&dir, Some(&name.to_string_lossy())).await;
    Ok(())
}

/// The folder in the server's `tmp/` where Forge's install steps from launcher-meta run.
const WORK_FOLDER: &str = "forge_installer_copal";

/// What an install leaves in the server folder: the vendor installer's log beside its jar, and
/// the temporary folders in `tmp/` it and the install steps work in. Removed through the folder,
/// so a link a mod planted is never followed; on Linux what the installer's account made and
/// only it may remove goes through that account. Best effort.
async fn remove_installer_leftovers(dir: &Path, installer: Option<&str>) {
    let dir = dir.to_owned();
    let log = installer.map(|installer| format!("{installer}.log"));
    let removed =
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            let folder = crate::content::folder::Folder::open(&dir)?;
            let mut leftovers: Vec<String> = log.into_iter().collect();
            if let Ok(tmp) = folder.open_dir(Some("tmp")) {
                for entry in tmp.entries()?.filter_map(Result::ok) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if leftover_name(&name) {
                        leftovers.push(format!("tmp/{name}"));
                    }
                }
            }
            for path in leftovers {
                if let Err(error) = folder.remove(&path) {
                    if error.kind() != std::io::ErrorKind::PermissionDenied {
                        return Err(error);
                    }
                    crate::servers::shim::java::remove_as_servers(
                        &dir.join(&path),
                    )?;
                }
            }
            Ok(())
        })
        .await;
    if let Err(error) = removed
        .map_err(std::io::Error::other)
        .and_then(|result| result)
    {
        tracing::warn!(%error, "could not remove what the installer left behind");
    }
}

/// A temporary folder of the Forge or NeoForge installer, or of the install steps
/// ([`WORK_FOLDER`]), by name.
fn leftover_name(name: &str) -> bool {
    name.starts_with("neoforge_installer")
        || name.starts_with("forge_installer")
}

#[cfg(test)]
mod leftover_tests {
    use super::*;

    #[tokio::test]
    async fn a_successful_installer_leaves_nothing_of_its_own_behind() {
        let dir = tempfile::tempdir().unwrap();
        let installer = "neoforge-21.1.77-installer.jar";
        std::fs::write(dir.path().join(format!("{installer}.log")), "log")
            .unwrap();
        std::fs::create_dir_all(
            dir.path().join("tmp/neoforge_installer123/libs"),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("tmp/neoforge_installer123/libs/a"),
            "x",
        )
        .unwrap();
        // The server's own temporary files stay.
        std::fs::write(dir.path().join("tmp/server.tmp"), "keep").unwrap();
        remove_installer_leftovers(dir.path(), Some(installer)).await;
        assert!(!dir.path().join(format!("{installer}.log")).exists());
        assert!(!dir.path().join("tmp/neoforge_installer123").exists());
        assert!(dir.path().join("tmp/server.tmp").exists());
    }
}
