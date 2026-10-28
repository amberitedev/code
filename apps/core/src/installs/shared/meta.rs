//! Modrinth's launcher-meta, read the way the App reads it (app-lib's `api::metadata` and
//! `launcher::get_loader_version_from_profile`), through the `minecraft-meta-manifest` types.
//! The merged version info is cached on disk by `version_info`; the manifests are small enough
//! to fetch per install.

use minecraft_meta_manifest::{
    fabriclike::{LoaderVersion, Manifest},
    minecraft::{CURRENT_FORMAT_VERSION, Catalog, Version},
    profile::{
        CURRENT_FABRIC_FORMAT_VERSION, CURRENT_FORGE_FORMAT_VERSION,
        CURRENT_NEOFORGE_FORMAT_VERSION, CURRENT_QUILT_FORMAT_VERSION,
        DUMMY_REPLACE_STRING,
    },
};

use crate::installs::{ModLoader, download};

/// Base URL of launcher-meta, app-lib's `MODRINTH_LAUNCHER_META_URL`.
const LAUNCHER_META_URL: &str = "https://launcher-meta.modrinth.com/";

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error("unknown game version: {0}")]
    UnknownGameVersion(String),
    #[error("{loader} has no {requested} build for Minecraft {game_version}")]
    NoLoaderVersion {
        loader: ModLoader,
        requested: String,
        game_version: String,
    },
}

/// Fetch the catalog of every Minecraft version.
pub async fn fetch_minecraft_manifest(
    http: &reqwest::Client,
) -> Result<Catalog, MetaError> {
    let url = format!(
        "{LAUNCHER_META_URL}minecraft/v{CURRENT_FORMAT_VERSION}/manifest.json"
    );
    Ok(download::json(http, &url).await?)
}

/// The catalog entry for `game_version`.
pub fn find_game_version(
    catalog: &Catalog,
    game_version: &str,
) -> Result<Version, MetaError> {
    catalog
        .versions
        .iter()
        .find(|version| version.id == game_version)
        .cloned()
        .ok_or_else(|| MetaError::UnknownGameVersion(game_version.into()))
}

/// Resolve `requested` for `loader` on `game_version` from its launcher-meta manifest. Every
/// loader, Forge and NeoForge included, shares one manifest shape and one rule.
pub async fn loader_version(
    http: &reqwest::Client,
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
) -> Result<LoaderVersion, MetaError> {
    let (path, format) = match loader {
        ModLoader::Fabric => ("fabric", CURRENT_FABRIC_FORMAT_VERSION),
        ModLoader::Quilt => ("quilt", CURRENT_QUILT_FORMAT_VERSION),
        ModLoader::Forge => ("forge", CURRENT_FORGE_FORMAT_VERSION),
        // launcher-meta calls NeoForge `neo`, as app-lib's `ModLoader::as_meta_str` does.
        ModLoader::NeoForge => ("neo", CURRENT_NEOFORGE_FORMAT_VERSION),
        ModLoader::Vanilla | ModLoader::Paper => {
            unreachable!("{loader} has no launcher-meta loader manifest")
        }
    };
    let url = format!("{LAUNCHER_META_URL}{path}/v{format}/manifest.json");
    let manifest: Manifest = download::json(http, &url).await?;
    resolve_loader_version(&manifest, loader, game_version, requested)
}

/// Pick a loader version the way the App's instance creation does. `requested` is a loader
/// version id, `latest` (the newest), or `stable` (the newest marked stable, else the newest);
/// `None` is `stable`, the App's default. launcher-meta marks no Forge or NeoForge build stable,
/// so for them `stable` is the newest.
pub fn resolve_loader_version(
    manifest: &Manifest,
    loader: ModLoader,
    game_version: &str,
    requested: Option<&str>,
) -> Result<LoaderVersion, MetaError> {
    let requested = requested.unwrap_or("stable");
    let missing = || MetaError::NoLoaderVersion {
        loader,
        requested: requested.into(),
        game_version: game_version.into(),
    };
    let game = manifest
        .game_versions
        .iter()
        .find(|version| {
            version.id.replace(DUMMY_REPLACE_STRING, game_version)
                == game_version
        })
        .ok_or_else(missing)?;
    // Newer manifests share one loader list between the versions of a group.
    let loaders = match &game.version_group {
        Some(group) => manifest
            .version_groups
            .iter()
            .find(|candidate| candidate.id == *group)
            .map_or(&[][..], |group| group.loaders.as_slice()),
        None => game.loaders.as_slice(),
    };
    loaders
        .iter()
        .find(|it| match requested {
            "latest" => true,
            "stable" => it.stable,
            id => it.id == id,
        })
        .or_else(|| (requested == "stable").then(|| loaders.first()).flatten())
        .cloned()
        .ok_or_else(missing)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> Manifest {
        serde_json::from_value(serde_json::json!({
            "gameVersions": [
                {
                    "id": "1.20.1",
                    "stable": true,
                    "loaders": [
                        {"id": "47.4.28", "url": "https://example.com/a.json", "stable": false},
                        {"id": "47.4.27", "url": "https://example.com/b.json", "stable": false}
                    ]
                },
                {
                    "id": "1.21.1",
                    "stable": true,
                    "versionGroup": "fabric",
                    "loaders": []
                }
            ],
            "versionGroups": [{
                "id": "fabric",
                "loaders": [
                    {"id": "0.17.0-beta.1", "url": "https://example.com/c.json", "stable": false},
                    {"id": "0.16.14", "url": "https://example.com/d.json", "stable": true}
                ]
            }]
        }))
        .unwrap()
    }

    #[test]
    fn versions_resolve_like_the_app_resolves_them() {
        let manifest = manifest();
        let pick = |game, requested| {
            resolve_loader_version(
                &manifest,
                ModLoader::Fabric,
                game,
                requested,
            )
            .map(|version| version.id)
        };
        // Stable is the default, and skips a newer beta.
        assert_eq!(pick("1.21.1", None).unwrap(), "0.16.14");
        assert_eq!(pick("1.21.1", Some("latest")).unwrap(), "0.17.0-beta.1");
        // Nothing marked stable, as on Forge: the newest.
        assert_eq!(pick("1.20.1", None).unwrap(), "47.4.28");
        assert_eq!(pick("1.20.1", Some("47.4.27")).unwrap(), "47.4.27");
        // A build launcher-meta does not list fails before anything downloads.
        assert!(matches!(
            pick("1.20.1", Some("47.2.0")),
            Err(MetaError::NoLoaderVersion { .. })
        ));
        assert!(pick("1.8.9", None).is_err());
    }
}
