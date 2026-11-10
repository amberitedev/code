//! A share version as the backend gives it and takes it.
//! `GET /v1/instances/:id/versions[/:n]` and `POST /v1/instances/:id/versions` answer with it,
//! and the invite preview carries one without file keys. Every field the backend always writes
//! is required, so a version that does not parse is refused as a whole.

use serde::{Deserialize, Serialize};

use super::RemovedFile;
use crate::pool::PoolCore;

/// One version of a share.
#[derive(Debug, Clone, Deserialize)]
pub struct ShareVersion {
    pub version: i32,
    pub ready: bool,
    #[serde(flatten)]
    pub content: VersionContent,
    /// R, the hash that names the version in the pool.
    #[serde(with = "hex32")]
    pub root: [u8; 32],
    pub external_files: Vec<ManifestFile>,
    /// Where readers fetch the files, once the version is ready and has some.
    #[serde(default)]
    pub pool: Option<PoolGrant>,
    /// Where the owner stores the files while the version is unconfirmed.
    #[serde(default)]
    pub store: Option<StoreGrant>,
    /// Upstream's marker for a version that starts members over. This backend never writes it,
    /// so it is always `None` here; the App still compares it as upstream does.
    #[serde(default)]
    pub backtrack_version: Option<String>,
}

/// A version's upstream content fields, which a push sends beside the files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VersionContent {
    pub modrinth_ids: Vec<String>,
    /// The Modrinth version of the modpack the version is based on.
    pub modpack_id: Option<String>,
    /// Files of the modpack that members do not get.
    pub removed_files: Vec<RemovedFile>,
    pub game_version: String,
    /// The loader's name as the backend takes it: `vanilla`, `fabric`, `forge`, `quilt` or
    /// `neoforge`.
    pub loader: String,
    /// Empty for vanilla.
    pub loader_version: String,
}

/// A file of the version that lives in the pool, as the manifest names it. Content files (mods
/// and the like) are named by their file name; every other file by its path in the instance or
/// server folder.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub file_name: String,
    pub file_type: String,
    pub file_size: u64,
    /// What a member's install compares to skip a file it already has.
    pub sha1: String,
    /// P, the SHA-256 of the plain bytes.
    #[serde(with = "hex32")]
    pub sha256: [u8; 32],
    /// C, the SHA-256 of the encrypted object.
    #[serde(with = "hex32")]
    pub object: [u8; 32],
    pub object_size: u64,
    /// The file's own key. Absent for content files, whose key derives from P, and in the
    /// invite preview, which carries no keys.
    #[serde(
        default,
        with = "hex32_option",
        skip_serializing_if = "Option::is_none"
    )]
    pub key: Option<[u8; 32]>,
}

/// The holders of a ready version and the fetch ticket for it.
#[derive(Debug, Clone, Deserialize)]
pub struct PoolGrant {
    pub holders: Vec<PoolCore>,
    pub ticket: String,
}

/// The Core the owner stores an unconfirmed version on, and the store ticket for it.
#[derive(Debug, Clone, Deserialize)]
pub struct StoreGrant {
    pub core: PoolCore,
    pub ticket: String,
}

impl ShareVersion {
    /// The manifest's config files: single files the owner picked, by their path.
    pub fn configs(&self) -> impl Iterator<Item = &ManifestFile> {
        self.external_files
            .iter()
            .filter(|file| file.file_type == crate::rules::CONFIG_FILE_TYPE)
    }

    /// The modpack the version is based on, when it has one.
    pub fn modpack_id(&self) -> Option<&str> {
        self.content
            .modpack_id
            .as_deref()
            .filter(|id| !id.is_empty())
    }

    /// Checks the removed files, which only a version based on a modpack may list.
    pub fn check_removed_files(&self) -> Result<(), super::InvalidRemovedFile> {
        if self.modpack_id().is_none() && !self.content.removed_files.is_empty()
        {
            return Err(super::InvalidRemovedFile);
        }
        self.content
            .removed_files
            .iter()
            .try_for_each(RemovedFile::validate)
    }
}

/// A 32-byte hash or key as lowercase hex.
pub(crate) mod hex32 {
    use serde::{Deserialize, Deserializer, Serializer, de::Error};

    pub fn serialize<S: Serializer>(
        bytes: &[u8; 32],
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&hex::encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<[u8; 32], D::Error> {
        let text = String::deserialize(deserializer)?;
        crate::crypto::parse_hash(&text)
            .ok_or_else(|| D::Error::custom("expected a 64-digit hex hash"))
    }
}

pub(crate) mod hex32_option {
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        bytes: &Option<[u8; 32]>,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        match bytes {
            Some(bytes) => super::hex32::serialize(bytes, serializer),
            None => serializer.serialize_none(),
        }
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        deserializer: D,
    ) -> Result<Option<[u8; 32]>, D::Error> {
        #[derive(Deserialize)]
        struct Hex(#[serde(with = "super::hex32")] [u8; 32]);
        Ok(Option::<Hex>::deserialize(deserializer)?.map(|Hex(bytes)| bytes))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_version_reads_as_the_backend_writes_it() {
        let hash = "ab".repeat(32);
        let body = serde_json::json!({
            "modrinth_ids": ["v1"],
            "modpack_id": null,
            "removed_files": [],
            "game_version": "1.21.1",
            "loader": "fabric",
            "loader_version": "0.16.0",
            "root": hash,
            "external_files": [{
                "file_name": "config/a.toml",
                "file_type": "config",
                "url": format!("pool:{hash}"),
                "file_size": 1,
                "sha1": "s",
                "sha256": hash,
                "object": hash,
                "object_size": 17,
                "key": hash,
            }],
            "version": 3,
            "ready": true,
        });
        let version: ShareVersion = serde_json::from_value(body).unwrap();
        assert_eq!(version.version, 3);
        assert_eq!(version.content.loader, "fabric");
        assert_eq!(version.configs().count(), 1);
        assert_eq!(version.external_files[0].key, Some([0xab; 32]));
        assert!(version.pool.is_none());

        let sent = serde_json::to_value(&version.external_files[0]).unwrap();
        assert_eq!(sent["sha256"], hash);
        assert!(sent.get("url").is_none());
    }

    #[test]
    fn a_content_file_carries_no_key() {
        let hash = "01".repeat(32);
        let file: ManifestFile = serde_json::from_value(serde_json::json!({
            "file_name": "a.jar",
            "file_type": "mod",
            "file_size": 1,
            "sha1": "s",
            "sha256": hash,
            "object": hash,
            "object_size": 17,
        }))
        .unwrap();
        assert_eq!(file.key, None);
        assert!(serde_json::to_value(&file).unwrap().get("key").is_none());
    }
}
