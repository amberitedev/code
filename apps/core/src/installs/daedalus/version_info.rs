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
use crate::installs::ModLoader;
use crate::installs::download;

#[derive(Debug, thiserror::Error)]
pub enum VersionInfoError {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
}

/// The merged version id the store keys a build by: `{game_version}` for Vanilla, otherwise
/// `{game_version}-{loader}-{loader_version}`, since two loaders can share a version number
/// (Fabric and Quilt 0.17.0 on 1.19.2).
pub fn merged_version_id(
    game_version: &str,
    loader: ModLoader,
    loader_version: Option<&str>,
) -> String {
    match loader_version.filter(|_| loader != ModLoader::Vanilla) {
        Some(version) => format!("{game_version}-{loader}-{version}"),
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
    loader: Option<(ModLoader, &LoaderVersion)>,
    force: bool,
) -> Result<VersionInfo, VersionInfoError> {
    let version_id = merged_version_id(
        &version.id,
        loader.map_or(ModLoader::Vanilla, |(loader, _)| loader),
        loader.map(|(_, version)| version.id.as_str()),
    );
    let loader = loader.map(|(_, version)| version);
    let path = dirs.version_info_path(&version_id);

    if path.exists() && !force {
        let bytes = tokio::fs::read(&path).await?;
        if let Ok(info) = serde_json::from_slice::<VersionInfo>(&bytes) {
            return Ok(info);
        }
        // Fall through and refetch on a corrupt cache entry.
    }

    let mut info: VersionInfo = download::json(http, &version.url).await?;

    if let Some(loader) = loader {
        let partial: PartialVersionInfo =
            download::json(http, &loader.url).await?;
        info = modded::merge_partial_version(partial, info);
    }

    info.id = version_id.clone();

    // Whole, so a server reading it at start never sees half a file.
    download::write_whole(&path, &serde_json::to_vec(&info)?).await?;

    Ok(info)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn two_loaders_on_one_version_number_never_share_metadata() {
        let fabric =
            merged_version_id("1.19.2", ModLoader::Fabric, Some("0.17.0"));
        let quilt =
            merged_version_id("1.19.2", ModLoader::Quilt, Some("0.17.0"));
        assert_eq!(fabric, "1.19.2-fabric-0.17.0");
        assert_eq!(quilt, "1.19.2-quilt-0.17.0");
        assert_eq!(
            merged_version_id("1.19.2", ModLoader::Vanilla, None),
            "1.19.2"
        );
    }
}
