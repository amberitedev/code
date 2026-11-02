//! `modrinth.index.json`, the index of a Modrinth `.mrpack`, as both sides read and write it.
//! Field names and values follow upstream's `PackFormat` (app-lib `api/pack/install_from.rs`),
//! which the App keeps for its own installs.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

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
    /// with its version. Read it with [`crate::content::platform`].
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

/// Whether a side takes a file. Mirrors app-lib's `SideType` (`state/cache.rs`).
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

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn an_unknown_side_still_reads() {
        let env: Env =
            serde_json::from_str(r#"{"server":"sometimes"}"#).unwrap();
        assert_eq!(env.client, SideType::Required);
        assert_eq!(env.server, SideType::Unknown);
    }
}
