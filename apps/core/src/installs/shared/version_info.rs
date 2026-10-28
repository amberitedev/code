//! A build's version info: Mojang's version JSON, with a Fabric or Quilt profile merged in.
//!
//! Port of app-lib's `launcher::download::download_version_info`, with `reqwest` and an on-disk
//! cache (`versions/{version_id}/{version_id}.json`) in place of app-lib's fetch cache. The vanilla
//! JSON of every game version Core installs is cached too, under its game version, so the Java a
//! server needs is known offline.

use minecraft_meta_manifest::{
    fabriclike::LoaderVersion,
    minecraft::{Version as GameVersion, VersionManifest},
    profile::{DUMMY_REPLACE_STRING, ProfileMetadata},
};
use serde::de::DeserializeOwned;

use super::dirs::SharedDirs;
use crate::installs::{ModLoader, download};

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

/// Mojang's version JSON for `version`, from the cache unless `force` (repair) refetches it.
pub async fn vanilla(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    version: &GameVersion,
    force: bool,
) -> Result<VersionManifest, VersionInfoError> {
    cached(dirs, &version.id, force, || {
        download::json(http, version.url.as_str())
    })
    .await
}

/// The version info a Vanilla, Fabric, or Quilt server launches from: Mojang's JSON, with the
/// loader's profile merged in when there is one. Cached under its merged version id.
pub async fn merged(
    http: &reqwest::Client,
    dirs: &SharedDirs,
    version: &GameVersion,
    loader: Option<(ModLoader, &LoaderVersion)>,
    force: bool,
) -> Result<VersionManifest, VersionInfoError> {
    let vanilla = vanilla(http, dirs, version, force).await?;
    let Some((loader, loader_version)) = loader else {
        return Ok(vanilla);
    };
    let id = merged_version_id(&version.id, loader, Some(&loader_version.id));
    cached(dirs, &id, force, || async {
        let profile: ProfileMetadata =
            download::json(http, loader_version.url.as_str()).await?;
        let mut info = merge_partial_version(profile, vanilla);
        info.id.0.clone_from(&id);
        Ok(info)
    })
    .await
}

/// `versions/{id}/{id}.json`, or what `fetch` returns, written whole so a server reading it at
/// start never sees half a file. A cache entry that no longer parses is fetched again.
async fn cached<T, F>(
    dirs: &SharedDirs,
    id: &str,
    force: bool,
    fetch: impl FnOnce() -> F,
) -> Result<T, VersionInfoError>
where
    T: DeserializeOwned + serde::Serialize,
    F: Future<Output = Result<T, download::DownloadError>>,
{
    let path = dirs.version_info_path(id);
    if !force
        && let Ok(bytes) = tokio::fs::read(&path).await
        && let Ok(info) = serde_json::from_slice(&bytes)
    {
        return Ok(info);
    }
    let info = fetch().await?;
    download::write_whole(&path, &serde_json::to_vec(&info)?).await?;
    Ok(info)
}

/// Merge a loader profile into Mojang's version JSON. app-lib's
/// `launcher::profile::merge_partial_version`, without the Forge processors a Fabric or Quilt
/// profile never has: a loader library replaces the vanilla one with the same group and name,
/// arguments are appended, and the loader's main class wins.
pub fn merge_partial_version(
    partial: ProfileMetadata,
    mut version: VersionManifest,
) -> VersionManifest {
    let game_version = version.id.0.clone();
    let artifact = |name: &str| {
        name.rsplit_once(':')
            .map(|(artifact, _)| artifact.to_owned())
    };
    version.libraries.retain(|library| {
        let replaced = artifact(&library.name);
        !partial.libraries.iter().any(|target| {
            target.include_in_classpath
                && replaced.is_some()
                && artifact(&target.name) == replaced
        })
    });
    version.libraries.extend(partial.libraries);
    for library in &mut version.libraries {
        library.name =
            library.name.replace(DUMMY_REPLACE_STRING, &game_version);
    }
    if let Some(arguments) = partial.arguments {
        let merged = version.arguments.get_or_insert_default();
        for (kind, arguments) in arguments {
            merged.entry(kind).or_default().extend(arguments);
        }
    }
    version.id.0 = partial.id.replace(DUMMY_REPLACE_STRING, &game_version);
    if let Some(main_class) = partial.main_class {
        version.main_class = main_class;
    }
    version.minecraft_arguments = partial.minecraft_arguments;
    version.release_time = partial.release_time;
    version.time = partial.time;
    version.ty = partial.ty;
    version
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

    #[test]
    fn a_loader_library_replaces_the_vanilla_one_of_the_same_name() {
        let library = |name: &str| {
            serde_json::json!({
                "name": name,
                "downloads": {"artifact": {
                    "sha1": "0000000000000000000000000000000000000000",
                    "size": 1,
                    "url": "https://libraries.minecraft.net/a.jar"
                }}
            })
        };
        let vanilla: VersionManifest = serde_json::from_value(serde_json::json!({
            "assetIndex": {
                "id": "17",
                "sha1": "0000000000000000000000000000000000000000",
                "size": 1,
                "totalSize": 1,
                "url": "https://piston-meta.mojang.com/17.json"
            },
            "assets": "17",
            "downloads": {},
            "id": "1.21.1",
            "libraries": [library("org.ow2.asm:asm:9.6"), library("com.google.guava:guava:32.1.2")],
            "mainClass": "net.minecraft.client.main.Main",
            "minimumLauncherVersion": 21,
            "releaseTime": "2024-08-08T12:24:45Z",
            "time": "2024-08-08T12:24:45Z",
            "type": "release"
        }))
        .unwrap();
        let profile: ProfileMetadata =
            serde_json::from_value(serde_json::json!({
                "id": "fabric-loader-0.16.14-${modrinth.gameVersion}",
                "inheritsFrom": "${modrinth.gameVersion}",
                "releaseTime": "2025-01-01T00:00:00+00:00",
                "time": "2025-01-01T00:00:00+00:00",
                "mainClass": "net.fabricmc.loader.impl.launch.knot.KnotClient",
                "libraries": [
                    library("org.ow2.asm:asm:9.8"),
                    library("net.fabricmc:intermediary:${modrinth.gameVersion}")
                ],
                "type": "release"
            }))
            .unwrap();
        let merged = merge_partial_version(profile, vanilla);
        let names: Vec<_> = merged
            .libraries
            .iter()
            .map(|library| library.name.as_str())
            .collect();
        assert_eq!(
            names,
            [
                "com.google.guava:guava:32.1.2",
                "org.ow2.asm:asm:9.8",
                "net.fabricmc:intermediary:1.21.1"
            ]
        );
        assert_eq!(merged.id, "fabric-loader-0.16.14-1.21.1");
        assert_eq!(
            merged.main_class,
            "net.fabricmc.loader.impl.launch.knot.KnotClient"
        );
    }
}
