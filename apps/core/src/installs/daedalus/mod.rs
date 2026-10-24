//! Global content-addressed shared store for Minecraft server installs.
//!
//! This is the server-side port of app-lib's (Theseus) shared client storage. The shared store
//! downloads each game version's libraries and server jar ONCE into `{data_dir}/meta/` and
//! launches every build with absolute classpaths into that single tree. A library or server jar
//! fetched for one build is reused by every other build that needs it.
//!
//! Only loaders whose server launch daedalus models directly are handled here — Vanilla, Fabric,
//! and Quilt. Forge and NeoForge use the vendor installer (`forge.rs`), and Paper its single jar
//! (`paper.rs`). Start builds the launch tokens from the cached version info (`launch.rs`).

pub mod dirs;
pub mod launch;
pub mod libraries;
pub mod meta;
pub mod rules;
pub mod version_info;

use daedalus::minecraft::{DownloadType, VersionInfo};
use daedalus::modded::LoaderVersion;

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
    #[error("no server download for {0}")]
    NoServerDownload(String),
    #[error("loader {0} is not handled by the shared store")]
    Unsupported(ModLoader),
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
    if !matches!(
        loader,
        ModLoader::Vanilla | ModLoader::Fabric | ModLoader::Quilt
    ) {
        return Err(SharedError::Unsupported(loader));
    }

    let mc_manifest = meta::fetch_minecraft_manifest(http).await?;
    let version = meta::find_game_version(&mc_manifest, game_version)?;
    let loader_ver =
        resolve_loader(http, loader, game_version, loader_version).await?;

    let version_info = version_info::download_version_info(
        http,
        dirs,
        &version,
        loader_ver.as_ref().map(|version| (loader, version)),
        verify,
    )
    .await?;
    let version_id = version_info::merged_version_id(
        game_version,
        loader,
        loader_ver.as_ref().map(|version| version.id.as_str()),
    );
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

    download_server_jar(http, dirs, &version_info, &version_id, verify).await?;
    progress(1.0);
    Ok(loader_ver.map(|loader| loader.id))
}

/// Resolve the loader version metadata for a modded build, or `None` for vanilla.
async fn resolve_loader(
    http: &reqwest::Client,
    loader: ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
) -> Result<Option<LoaderVersion>, SharedError> {
    if loader == ModLoader::Vanilla {
        return Ok(None);
    }
    let manifest =
        meta::fetch_loader_manifest(http, meta::meta_loader_str(loader))
            .await?;
    let resolved =
        meta::resolve_loader_version(&manifest, game_version, loader_version)?;
    Ok(Some(resolved))
}

/// Download the vanilla server jar from the merged version info into the shared store, checked
/// against its SHA-1. A jar that is there is kept unless `verify` finds it damaged.
async fn download_server_jar(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    version_info: &VersionInfo,
    version_id: &str,
    verify: bool,
) -> Result<(), SharedError> {
    let download = version_info
        .downloads
        .get(&DownloadType::Server)
        .ok_or_else(|| SharedError::NoServerDownload(version_id.to_string()))?;
    let path = dirs.server_jar_path(version_id);
    if path.exists()
        && (!verify
            || download::has_hash(&path, Hash::Sha1(&download.sha1)).await)
    {
        return Ok(());
    }
    download::file(
        http,
        &download.url,
        &path,
        &FileDownload {
            hash: Hash::Sha1(&download.sha1),
            limit: None,
            progress: &download::quiet,
        },
    )
    .await?;
    Ok(())
}
