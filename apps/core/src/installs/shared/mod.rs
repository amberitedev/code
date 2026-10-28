//! Global content-addressed shared store for Minecraft server installs.
//!
//! This is the server-side port of app-lib's (Theseus) shared client storage. The shared store
//! downloads each game version's libraries and server jar ONCE into `{data_dir}/meta/` and
//! launches every build with absolute classpaths into that single tree. A library or server jar
//! fetched for one build is reused by every other build that needs it.
//!
//! All metadata comes from Modrinth's launcher-meta, read with the `minecraft-meta-manifest`
//! types the App uses. Vanilla, Fabric, and Quilt are installed here; Forge and NeoForge put
//! their libraries and install steps' results in the same store (`forge.rs`), and Paper keeps its
//! single jar (`paper.rs`). Start builds the launch tokens from the cached version info
//! (`launch.rs`).

pub mod dirs;
pub mod launch;
pub mod libraries;
pub mod meta;
pub mod rules;
pub mod version_info;

use minecraft_meta_manifest::minecraft::{DownloadType, VersionManifest};

pub use dirs::SharedDirs;

use super::download::{self, FileDownload, Hash};
use crate::installs::ModLoader;

#[derive(Debug, thiserror::Error)]
pub enum SharedError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error(transparent)]
    Meta(#[from] meta::MetaError),
    #[error("version info: {0}")]
    VersionInfo(#[from] version_info::VersionInfoError),
    #[error("library: {0}")]
    Library(#[from] libraries::LibraryError),
    #[error("Minecraft {0} has no server download")]
    NoServerDownload(String),
}

/// Install a Vanilla, Fabric, or Quilt build into the shared store and return the loader version
/// it resolved (`None` for Vanilla). `verify` (repair) re-hashes what is already there and
/// replaces only what does not match; nothing is ever deleted, so a running server keeps every
/// file it uses. `progress` is told the share done.
pub async fn install_shared(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    loader: ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
    verify: bool,
    progress: &(dyn Fn(f64) + Sync),
) -> Result<Option<String>, SharedError> {
    let catalog = meta::fetch_minecraft_manifest(http).await?;
    let version = meta::find_game_version(&catalog, game_version)?;
    let loader_version = match loader {
        ModLoader::Fabric | ModLoader::Quilt => Some(
            meta::loader_version(http, loader, game_version, loader_version)
                .await?,
        ),
        _ => None,
    };
    let version_info = version_info::merged(
        http,
        dirs,
        &version,
        loader_version.as_ref().map(|version| (loader, version)),
        verify,
    )
    .await?;
    progress(0.05);

    libraries::download_libraries(
        http,
        dirs,
        &version_info.libraries,
        std::env::consts::ARCH,
        verify,
        &|share| progress(0.05 + share * 0.75),
    )
    .await?;

    download_server_jar(http, dirs, &version_info, game_version, verify)
        .await?;
    progress(1.0);
    Ok(loader_version.map(|loader| loader.id))
}

/// Download the vanilla server jar of `game_version` into the shared store, checked against its
/// SHA-1. A jar that is there is kept unless `verify` finds it damaged.
pub async fn download_server_jar(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    version_info: &VersionManifest,
    game_version: &str,
    verify: bool,
) -> Result<(), SharedError> {
    let download = version_info
        .downloads
        .get(&DownloadType::Server)
        .ok_or_else(|| SharedError::NoServerDownload(game_version.into()))?;
    let sha1 = download.sha1.to_string();
    let path = dirs.server_jar_path(game_version);
    if path.exists()
        && (!verify || download::has_hash(&path, Hash::Sha1(&sha1)).await)
    {
        return Ok(());
    }
    download::file(
        http,
        download.url.as_str(),
        &path,
        &FileDownload {
            hash: Hash::Sha1(&sha1),
            limit: None,
            progress: &download::quiet,
        },
    )
    .await?;
    Ok(())
}
