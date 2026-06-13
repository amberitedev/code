//! Download and merge of a build's full version info.
//!
//! Port of app-lib's `launcher::download::download_version_info`, adapted to use
//! `reqwest` directly and an on-disk cache (`{version_id}.json`) instead of
//! app-lib's DB-backed fetch cache.
//!
//! The merge combines the vanilla Mojang version JSON with the loader's
//! `PartialVersionInfo` (Fabric/Quilt/Forge/NeoForge) via daedalus's
//! `merge_partial_version`, producing a single `VersionInfo` whose `libraries`,
//! `mainClass`, `arguments`, and (for Forge) `processors`/`data` are unified.

use daedalus::minecraft::{Version as GameVersion, VersionInfo};
use daedalus::modded::{self, LoaderVersion, PartialVersionInfo};

use super::dirs::SharedDirs;

#[derive(Debug, thiserror::Error)]
pub enum VersionInfoError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// The merged version id: `{game_version}` for vanilla, otherwise
/// `{game_version}-{loader_version}`.
pub fn merged_version_id(
    game_version: &str,
    loader: Option<&LoaderVersion>,
) -> String {
    match loader {
        Some(l) => format!("{game_version}-{}", l.id),
        None => game_version.to_string(),
    }
}

/// Download (or load from the on-disk cache) the merged version info for a build.
///
/// When `force` is true the cache is ignored and the metadata is refetched —
/// used by repair to recover from a corrupt cached JSON.
pub async fn download_version_info(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    version: &GameVersion,
    loader: Option<&LoaderVersion>,
    force: bool,
) -> Result<VersionInfo, VersionInfoError> {
    let version_id = merged_version_id(&version.id, loader);
    let path = dirs.version_info_path(&version_id);

    if path.exists() && !force {
        let bytes = tokio::fs::read(&path).await?;
        if let Ok(info) = serde_json::from_slice::<VersionInfo>(&bytes) {
            return Ok(info);
        }
        // Fall through and refetch on a corrupt cache entry.
    }

    let mut info: VersionInfo = http
        .get(&version.url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    if let Some(loader) = loader {
        let partial: PartialVersionInfo = http
            .get(&loader.url)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;
        info = modded::merge_partial_version(partial, info);
    }

    info.id = version_id.clone();

    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    tokio::fs::write(&path, serde_json::to_vec(&info)?).await?;

    Ok(info)
}
