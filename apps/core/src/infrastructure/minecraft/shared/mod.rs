//! Global content-addressed shared store for Minecraft server installs.
//!
//! This is the server-side port of app-lib's (Theseus) shared client storage.
//! Where the legacy installer downloaded an opaque blob into each instance's
//! directory, the shared store downloads each game version's libraries and
//! server jar ONCE into `{data_dir}/meta/` and launches every build with
//! absolute classpaths into that single tree. A library or server jar fetched
//! for one build is reused by every other build that needs it.
//!
//! Only loaders whose server launch daedalus models directly are handled here —
//! Vanilla, Fabric, and Quilt. Forge/NeoForge (BootstrapLauncher module path,
//! generated args files, SIDE=server processors) and Paper (URL jar) stay on the
//! vendor installer/flavours path in `installer.rs`/`flavours.rs`.
//!
//! `install_shared` returns the absolute, instance-independent launch tokens
//! (the portion of the JVM command between the memory flags and the user's extra
//! args). They are persisted in the installation's `launch.json` as
//! `LaunchStyle::Modular` and replayed verbatim at start time.

pub mod dirs;
pub mod launch;
pub mod libraries;
pub mod meta;
pub mod processors;
pub mod rules;
pub mod version_info;

use std::path::Path;

use daedalus::minecraft::{Download, DownloadType, VersionInfo};
use daedalus::modded::LoaderVersion;
use sha1::Digest;

pub use dirs::SharedDirs;

use crate::domain::instance::ModLoader;

#[derive(Debug, thiserror::Error)]
pub enum SharedError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("meta: {0}")]
    Meta(#[from] meta::MetaError),
    #[error("version info: {0}")]
    VersionInfo(#[from] version_info::VersionInfoError),
    #[error("library: {0}")]
    Library(#[from] libraries::LibraryError),
    #[error("processor: {0}")]
    Processor(#[from] processors::ProcessorError),
    #[error("no server download for {0}")]
    NoServerDownload(String),
    #[error("server jar sha1 mismatch: expected {expected}, got {actual}")]
    HashMismatch { expected: String, actual: String },
    #[error("loader {0:?} is not handled by the shared store")]
    Unsupported(ModLoader),
}

/// Whether a loader's server install is handled by the daedalus shared store.
pub fn is_shared_loader(loader: &ModLoader) -> bool {
    matches!(
        loader,
        ModLoader::Vanilla | ModLoader::Fabric | ModLoader::Quilt
    )
}

/// Install a Vanilla/Fabric/Quilt build into the global shared store and return
/// its absolute launch tokens.
///
/// All artifacts land under `{data_dir}/meta/` and are deduped across every
/// installation. `installation_dir` is unused by the store itself — the caller
/// persists the returned tokens into that directory's `launch.json`.
pub async fn install_shared(
    http: &reqwest::Client,
    data_dir: &Path,
    loader: &ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
    java_path: Option<&Path>,
    java_arch: &str,
) -> Result<Vec<String>, SharedError> {
    if !is_shared_loader(loader) {
        return Err(SharedError::Unsupported(loader.clone()));
    }

    let dirs = SharedDirs::new(data_dir);

    let mc_manifest = meta::fetch_minecraft_manifest(http).await?;
    let version = meta::find_game_version(&mc_manifest, game_version)?;

    let loader_ver =
        resolve_loader(http, loader, game_version, loader_version).await?;

    let version_info = version_info::download_version_info(
        http,
        &dirs,
        &version,
        loader_ver.as_ref(),
        false,
    )
    .await?;
    let version_id =
        version_info::merged_version_id(game_version, loader_ver.as_ref());

    libraries::download_libraries(
        http,
        &dirs,
        &version_info.libraries,
        java_arch,
        false,
    )
    .await?;

    download_server_jar(http, &dirs, &version_info, &version_id).await?;

    // Vanilla/Fabric/Quilt declare no processors, but run defensively so the
    // path is correct the day a shared loader does (and to mirror app-lib).
    let java = java_path.unwrap_or_else(|| Path::new("java"));
    processors::run_processors(
        java,
        &version_info,
        dirs.root(),
        &dirs.libraries_dir(),
        &dirs.server_jar_path(&version_id),
        game_version,
        java_arch,
    )
    .await?;

    launch::build_launch_tokens(
        &dirs,
        &version_info,
        &version_id,
        loader,
        java_arch,
    )?
    .ok_or_else(|| SharedError::Unsupported(loader.clone()))
}

/// Resolve the loader version metadata for a modded build, or `None` for vanilla.
async fn resolve_loader(
    http: &reqwest::Client,
    loader: &ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
) -> Result<Option<LoaderVersion>, SharedError> {
    if matches!(loader, ModLoader::Vanilla) {
        return Ok(None);
    }
    let meta_loader = meta::meta_loader_str(loader);
    let manifest = meta::fetch_loader_manifest(http, meta_loader).await?;
    let resolved =
        meta::resolve_loader_version(&manifest, game_version, loader_version)?;
    Ok(Some(resolved))
}

/// Download the vanilla server jar from the merged version info into the shared
/// store, verifying its SHA1. Deduped: an existing jar is left untouched.
async fn download_server_jar(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    version_info: &VersionInfo,
    version_id: &str,
) -> Result<(), SharedError> {
    let path = dirs.server_jar_path(version_id);
    if path.exists() {
        return Ok(());
    }

    let download: &Download = version_info
        .downloads
        .get(&DownloadType::Server)
        .ok_or_else(|| SharedError::NoServerDownload(version_id.to_string()))?;

    let bytes = http
        .get(&download.url)
        .send()
        .await?
        .error_for_status()?
        .bytes()
        .await?;

    let actual = hex::encode(sha1::Sha1::digest(&bytes));
    if !download.sha1.is_empty() && actual != download.sha1 {
        return Err(SharedError::HashMismatch {
            expected: download.sha1.clone(),
            actual,
        });
    }

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, &bytes).await?;
    Ok(())
}
