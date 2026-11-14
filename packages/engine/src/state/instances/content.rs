use super::ContentSourceKind;
use crate::state::{
    License, Project, ProjectType, Version, VersionEnvironment,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ContentItem {
    pub file_name: String,
    pub file_path: String,
    pub id: String,
    pub size: u64,
    pub enabled: bool,
    pub locked: bool,
    pub project_type: ProjectType,
    pub project: Option<ContentItemProject>,
    pub version: Option<ContentItemVersion>,
    pub environment: Option<VersionEnvironment>,
    pub owner: Option<ContentItemOwner>,
    pub has_update: bool,
    pub update_version_id: Option<String>,
    pub date_added: Option<String>,
    pub source_kind: Option<ContentSourceKind>,
    pub embedded_metadata: Option<EmbeddedContentMetadata>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub synced_pack: Option<SyncedPackInfo>,
    /// Where the addon runs, on a server engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server: Option<ServerSides>,
}

/// Where an addon of a server runs.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, Default)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ServerSides {
    /// Off for the server alone: its file is `.disabled`, and players still get it.
    pub disabled_server: bool,
    /// Off for players: the server runs it, and its share leaves it out.
    pub disabled_player: bool,
    /// Someone set the sides by hand.
    pub sides_unlocked: bool,
    /// A client-only file of the pack someone turned on for the server.
    pub server_override: bool,
    /// False for a client-only file of the pack, which is never on the server's disk.
    pub installed: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SyncedPackInfo {
    pub id: String,
    pub instance_ids: Vec<String>,
    pub update_pending: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct EmbeddedContentMetadata {
    pub name: Option<String>,
    pub version: Option<String>,
    pub icon_path: Option<String>,
}

impl EmbeddedContentMetadata {
    pub fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.version.is_none()
            && self.icon_path.is_none()
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ContentItemProject {
    pub id: String,
    pub slug: Option<String>,
    pub title: String,
    pub icon_url: Option<String>,
    pub license: License,
    pub categories: Vec<String>,
    pub additional_categories: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ContentItemVersion {
    pub id: String,
    pub version_number: String,
    pub file_name: String,
    pub date_published: Option<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ContentItemOwner {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
    #[serde(rename = "type")]
    pub owner_type: OwnerType,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum OwnerType {
    User,
    Organization,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct LinkedModpackInfo {
    pub project: Project,
    pub version: Option<Version>,
    pub owner: Option<ContentItemOwner>,
    pub has_update: bool,
    pub update_version_id: Option<String>,
    pub update_version: Option<Version>,
}
