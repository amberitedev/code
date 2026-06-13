//! Modrinth launcher-meta client for the shared store.
//!
//! Mirrors app-lib's `api::metadata` but without app-lib's DB-backed cache: the
//! merged version info is cached on disk by `version_info`, and the manifests
//! are small enough to fetch per install. All metadata comes from Modrinth's
//! launcher-meta mirror, which already merges Mojang + loader data into the
//! daedalus model types.

use daedalus::minecraft::VersionManifest;
use daedalus::modded::{LoaderVersion, Manifest, DUMMY_REPLACE_STRING};

use crate::domain::instance::ModLoader;

/// Base URL of the Modrinth launcher-meta mirror (matches app-lib's
/// `MODRINTH_LAUNCHER_META_URL`).
const LAUNCHER_META_URL: &str = "https://launcher-meta.modrinth.com/";

#[derive(Debug, thiserror::Error)]
pub enum MetaError {
    #[error("http: {0}")]
    Http(#[from] reqwest::Error),
    #[error("unknown game version: {0}")]
    UnknownGameVersion(String),
    #[error("no loader version found for {loader} on {game_version}")]
    NoLoaderVersion {
        loader: String,
        game_version: String,
    },
}

/// The launcher-meta path segment for a loader. Note NeoForge is `neo`, matching
/// app-lib's `ModLoader::as_meta_str`.
pub fn meta_loader_str(loader: &ModLoader) -> &'static str {
    match loader {
        ModLoader::Vanilla | ModLoader::Paper => "vanilla",
        ModLoader::Forge => "forge",
        ModLoader::Fabric => "fabric",
        ModLoader::Quilt => "quilt",
        ModLoader::NeoForge => "neo",
    }
}

/// Fetch the full Minecraft version manifest (every released game version).
pub async fn fetch_minecraft_manifest(
    http: &reqwest::Client,
) -> Result<VersionManifest, MetaError> {
    let url = format!(
        "{LAUNCHER_META_URL}minecraft/v{}/manifest.json",
        daedalus::minecraft::CURRENT_FORMAT_VERSION
    );
    Ok(http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

/// Fetch a loader's manifest (its supported game versions + loader versions).
pub async fn fetch_loader_manifest(
    http: &reqwest::Client,
    meta_loader: &str,
) -> Result<Manifest, MetaError> {
    let url = format!("{LAUNCHER_META_URL}{meta_loader}/v0/manifest.json");
    Ok(http
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?)
}

/// Resolve the daedalus `Version` (game version metadata) for `game_version`.
pub fn find_game_version(
    manifest: &VersionManifest,
    game_version: &str,
) -> Result<daedalus::minecraft::Version, MetaError> {
    manifest
        .versions
        .iter()
        .find(|v| v.id == game_version)
        .cloned()
        .ok_or_else(|| MetaError::UnknownGameVersion(game_version.to_string()))
}

/// Pick a loader version for the given game version from a loader manifest.
///
/// `requested` accepts a concrete loader version id, `"latest"`, or `"stable"`.
/// `None` is treated as `"latest"`. Mirrors app-lib's
/// `get_loader_version_from_profile` selection logic, including the
/// `DUMMY_REPLACE_STRING` game-version substitution loaders use in their ids.
pub fn resolve_loader_version(
    manifest: &Manifest,
    game_version: &str,
    requested: Option<&str>,
) -> Result<LoaderVersion, MetaError> {
    let requested = requested.unwrap_or("latest");

    let game = manifest
        .game_versions
        .iter()
        .find(|v| {
            v.id.replace(DUMMY_REPLACE_STRING, game_version) == game_version
        })
        .ok_or_else(|| MetaError::NoLoaderVersion {
            loader: "<unknown>".to_string(),
            game_version: game_version.to_string(),
        })?;

    let filter = |it: &&LoaderVersion| match requested {
        "latest" => true,
        "stable" => it.stable,
        id => it.id == id,
    };

    game.loaders
        .iter()
        .find(filter)
        .or_else(|| {
            if requested == "stable" {
                game.loaders.first()
            } else {
                None
            }
        })
        .cloned()
        .ok_or_else(|| MetaError::NoLoaderVersion {
            loader: requested.to_string(),
            game_version: game_version.to_string(),
        })
}
