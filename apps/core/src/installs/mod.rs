//! Server installs: the shared Daedalus store for Vanilla, Fabric, and Quilt, the vendor installer
//! for Forge and NeoForge, Paper's jar, and the Java runtimes Core manages. Everything shared
//! lives in `meta/` and `java/`, which servers may read but not write.

pub mod daedalus;
pub mod download;
pub mod forge;
pub mod java;
pub mod paper;

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::servers::{Hold, ServerError, Servers, launch, records::ServerRow};
use daedalus::SharedDirs;

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
    Shared(#[from] daedalus::SharedError),
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
    let java = launch::java_for(paths, http, &target).await?;
    progress(0.2);
    let rest = |share: f64| progress(0.2 + share * 0.8);
    let installed = match loader {
        ModLoader::Vanilla | ModLoader::Fabric | ModLoader::Quilt => {
            daedalus::install_shared(
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
            let installer =
                forge::installer(http, &dirs, loader, game_version, &version)
                    .await?;
            rest(0.1);
            run_vendor_installer(servers, hold, &installer, java).await?;
            forge::launch_args(
                &paths.server(hold.id()),
                loader,
                game_version,
                &version,
            )?;
            Some(version)
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
    Ok(result?)
}
