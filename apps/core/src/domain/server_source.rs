//! Linked server sources. Mirrors "Linked servers" in
//! `packages/api-client/src/modules/core/types.ts`.

use serde::{Deserialize, Serialize};

use super::instance::ModLoader;

/// Where a linked server's base setup comes from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerSource {
    Instance {
        shared_instance_id: String,
        name: String,
        icon: Option<String>,
    },
    Modrinth {
        project_id: String,
        version_id: String,
    },
    Mrpack {
        filename: String,
        name: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceApplyState {
    UpToDate,
    Downloading,
    Pending,
    Applying,
    Failed,
}

impl SourceApplyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UpToDate => "up_to_date",
            Self::Downloading => "downloading",
            Self::Pending => "pending",
            Self::Applying => "applying",
            Self::Failed => "failed",
        }
    }

    pub fn parse(value: &str) -> Self {
        match value {
            "up_to_date" => Self::UpToDate,
            "downloading" => Self::Downloading,
            "pending" => Self::Pending,
            "applying" => Self::Applying,
            _ => Self::Failed,
        }
    }
}

/// GET /instances/:id/source
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerSourceStatus {
    pub source: ServerSource,
    pub desired_version: String,
    pub installed_version: Option<String>,
    pub state: SourceApplyState,
    pub error: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SourceApplyPhase {
    Downloading,
    Verifying,
    WaitingForStop,
    Stopping,
    Applying,
    Starting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentKind {
    Mod,
    Datapack,
    /// Any other source file, such as pack config overrides. Tracked, not listed.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContentOrigin {
    Source,
    Server,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerContentItem {
    pub kind: ContentKind,
    pub filename: String,
    pub origin: ContentOrigin,
    pub client_only: bool,
    pub disabled: bool,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub name: Option<String>,
    pub icon_url: Option<String>,
}

/// GET /instances/:id/content
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerContent {
    pub source: Option<ServerSourceStatus>,
    pub items: Vec<ServerContentItem>,
}

/// A complete, verified source version as produced by an adapter. Stored as `setup.json` next
/// to the staged files; every adapter (mrpack, Modrinth, shared instance) produces this shape.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Setup {
    pub name: String,
    pub version_number: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub files: Vec<SetupFile>,
}

/// One file the source provides. `path` is relative to the instance data dir.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetupFile {
    pub path: String,
    pub kind: ContentKind,
    /// SHA-1 of the file. For client-only files, the source's declared hash (may be empty).
    pub sha1: String,
    /// Server env is unsupported: listed, never written to the server.
    pub client_only: bool,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
}
