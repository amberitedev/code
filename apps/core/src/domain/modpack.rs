use serde::{Deserialize, Serialize};

/// Root of a `.mrpack` index file (`modrinth.index.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFormat {
    pub game: String,
    pub name: String,
    pub version_id: String,
    pub summary: Option<String>,
    pub files: Vec<PackFile>,
    pub dependencies: std::collections::HashMap<String, String>,
}

/// A single file entry in a `.mrpack`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub hashes: PackFileHashes,
    pub env: Option<PackFileEnv>,
    pub downloads: Vec<String>,
    pub file_size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFileHashes {
    pub sha1: Option<String>,
    pub sha512: Option<String>,
}

/// Per-side environment requirement for a file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackFileEnv {
    pub client: EnvType,
    pub server: EnvType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EnvType {
    Required,
    Optional,
    Unsupported,
}

/// Link to Modrinth project/version for an installed modpack.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedData {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}

/// Persisted modpack manifest for an instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModpackManifest {
    pub id: String,
    pub instance_id: String,
    pub pack_name: String,
    pub pack_version: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub modrinth_project_id: Option<String>,
    pub modrinth_version_id: Option<String>,
    pub installed_at: String,
}
