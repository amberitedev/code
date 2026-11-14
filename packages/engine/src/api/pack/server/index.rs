//! `modrinth.index.json`, the index of a Modrinth `.mrpack`, as a server engine reads it. Field
//! names and values follow upstream's `PackFormat` (`install_from.rs`), which desktop installs
//! keep.

use std::{
    collections::BTreeMap,
    io::{Read, Seek},
};

use serde::{Deserialize, Serialize};

use super::PackError;
use crate::state::ModLoader;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    pub format_version: u32,
    pub game: String,
    pub version_id: String,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    #[serde(default)]
    pub files: Vec<IndexFile>,
    /// `minecraft` and the loader (`fabric-loader`, `quilt-loader`, `forge`, `neoforge`), each
    /// with its version. Read it with [`platform`].
    #[serde(default)]
    pub dependencies: BTreeMap<String, String>,
}

impl Index {
    /// The only format and game a pack can have.
    pub const FORMAT_VERSION: u32 = 1;
    pub const GAME: &str = "minecraft";
}

/// A file the pack downloads.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndexFile {
    pub path: String,
    pub hashes: Hashes,
    /// Which sides take the file. Missing means both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub env: Option<Env>,
    #[serde(default)]
    pub downloads: Vec<String>,
    pub file_size: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hashes {
    pub sha1: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Env {
    #[serde(default)]
    pub client: SideType,
    #[serde(default)]
    pub server: SideType,
}

/// Whether a side takes a file.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum SideType {
    #[default]
    Required,
    Optional,
    Unsupported,
    #[serde(other)]
    Unknown,
}

/// The index is small; a pack claiming a larger one is refused.
const MAX_INDEX: u64 = 32 * 1024 * 1024;

/// Read and check a pack's index.
pub fn read_index<R: Read + Seek>(
    archive: &mut zip::ZipArchive<R>,
) -> Result<Index, PackError> {
    let not_a_pack = |reason: &str| PackError::NotAPack(reason.to_owned());
    let mut text = String::new();
    archive
        .by_name("modrinth.index.json")
        .map_err(|_| not_a_pack("it has no modrinth.index.json"))?
        .take(MAX_INDEX)
        .read_to_string(&mut text)
        .map_err(|_| not_a_pack("its index is unreadable"))?;
    let index: Index = serde_json::from_str(&text)
        .map_err(|error| PackError::NotAPack(error.to_string()))?;
    if index.format_version != Index::FORMAT_VERSION
        || index.game != Index::GAME
    {
        return Err(not_a_pack("it is not a Minecraft pack of format 1"));
    }
    Ok(index)
}

/// Each loader's key in a pack's `dependencies`. Older packs spell NeoForge `neo-forge`, which
/// upstream also reads.
const LOADER_DEPENDENCIES: [(ModLoader, &str); 5] = [
    (ModLoader::Fabric, "fabric-loader"),
    (ModLoader::Quilt, "quilt-loader"),
    (ModLoader::Forge, "forge"),
    (ModLoader::NeoForge, "neoforge"),
    (ModLoader::NeoForge, "neo-forge"),
];

/// The loader and versions a pack runs on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Platform {
    pub loader: ModLoader,
    pub game_version: String,
    pub loader_version: Option<String>,
}

/// The platform a pack runs on; one that names no Minecraft version is not a pack.
pub fn platform(index: &Index) -> Result<Platform, PackError> {
    let game_version = index
        .dependencies
        .get("minecraft")
        .cloned()
        .ok_or_else(|| {
            PackError::NotAPack("it names no Minecraft version".into())
        })?;
    let (loader, loader_version) = LOADER_DEPENDENCIES
        .iter()
        .find_map(|(loader, key)| {
            index
                .dependencies
                .get(*key)
                .map(|version| (*loader, Some(version.clone())))
        })
        .unwrap_or((ModLoader::Vanilla, None));
    Ok(Platform {
        loader,
        game_version,
        loader_version,
    })
}

/// The project and version of a file on Modrinth's CDN, from its URL:
/// `https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`.
pub fn ids_from_cdn(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://cdn.modrinth.com/data/")?;
    let mut parts = rest.split('/');
    let project = parts.next()?;
    (parts.next()? == "versions").then_some(())?;
    let version = parts.next()?;
    parts.next()?;
    Some((project.to_owned(), version.to_owned()))
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Write};

    use zip::write::SimpleFileOptions;

    use super::*;

    fn pack(index: &str) -> zip::ZipArchive<Cursor<Vec<u8>>> {
        let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
        writer
            .start_file("modrinth.index.json", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(index.as_bytes()).unwrap();
        zip::ZipArchive::new(writer.finish().unwrap()).unwrap()
    }

    #[test]
    fn only_minecraft_packs_of_format_1_are_read() {
        let index = read_index(&mut pack(
            r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Pack","dependencies":{"minecraft":"1.21.1"}}"#,
        ))
        .unwrap();
        assert_eq!(index.name, "Pack");
        assert!(
            read_index(&mut pack(
                r#"{"formatVersion":2,"game":"minecraft","versionId":"1","name":"Pack"}"#
            ))
            .is_err()
        );
        assert!(read_index(&mut pack("{")).is_err());
    }

    #[test]
    fn an_index_round_trips_in_upstream_s_shape() {
        let text = r#"{"formatVersion":1,"game":"minecraft","versionId":"1","name":"Pack","files":[{"path":"mods/a.jar","hashes":{"sha1":"aa","sha512":"bb"},"env":{"client":"required","server":"unsupported"},"downloads":["https://cdn.modrinth.com/a.jar"],"fileSize":3}],"dependencies":{"fabric-loader":"0.16.0","minecraft":"1.21.1"}}"#;
        let index: Index = serde_json::from_str(text).unwrap();
        assert_eq!(
            index.files[0].env,
            Some(Env {
                client: SideType::Required,
                server: SideType::Unsupported,
            })
        );
        assert_eq!(serde_json::to_string(&index).unwrap(), text);
        let env: Env =
            serde_json::from_str(r#"{"server":"sometimes"}"#).unwrap();
        assert_eq!(env.client, SideType::Required);
        assert_eq!(env.server, SideType::Unknown);
    }

    #[test]
    fn a_pack_names_its_loader_and_versions() {
        let index = |dependencies: &[(&str, &str)]| Index {
            format_version: 1,
            game: "minecraft".into(),
            version_id: "1".into(),
            name: "Pack".into(),
            summary: None,
            files: Vec::new(),
            dependencies: dependencies
                .iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect(),
        };
        assert_eq!(
            platform(&index(&[
                ("minecraft", "1.21.1"),
                ("neo-forge", "21.1.1")
            ]))
            .unwrap(),
            Platform {
                loader: ModLoader::NeoForge,
                game_version: "1.21.1".into(),
                loader_version: Some("21.1.1".into()),
            }
        );
        assert_eq!(
            platform(&index(&[("minecraft", "1.21.1")])).unwrap().loader,
            ModLoader::Vanilla
        );
        assert!(platform(&index(&[("fabric-loader", "0.16.0")])).is_err());
    }

    #[test]
    fn cdn_urls_name_their_project_and_version() {
        assert_eq!(
            ids_from_cdn(
                "https://cdn.modrinth.com/data/AANobbMI/versions/u6dRKJwZ/sodium.jar"
            ),
            Some(("AANobbMI".into(), "u6dRKJwZ".into()))
        );
        assert_eq!(ids_from_cdn("https://github.com/a/b/releases/x.jar"), None);
        assert_eq!(
            ids_from_cdn("https://cdn.modrinth.com/data/AANobbMI/icon.png"),
            None
        );
    }
}
