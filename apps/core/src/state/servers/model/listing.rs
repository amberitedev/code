//! The content list and the pack card, as the App's Archon types name them, which
//! `commands::list_content` builds and `api::content` answers with; and the edited files of
//! `commands::edited_files`.

use bedrock::content::VersionEnvironment;
use chrono::{DateTime, Utc};
use modrinth_content_management::ContentType;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// Mirrors Archon.Content.v1.ContentOwnerType.
#[allow(dead_code)] // Core sends no owners yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ContentOwnerType {
    User,
    Organization,
}

/// The Modrinth user or organization behind a project. Mirrors Archon.Content.v1.ContentOwner, and
/// Archon.Content.v1.ModpackOwner, which has the same shape.
#[derive(Debug, Clone, Serialize)]
pub struct ContentOwner {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: ContentOwnerType,
    pub icon_url: Option<String>,
}

/// Mirrors Archon.Content.v1.AddonVersion.
#[derive(Debug, Clone, Serialize)]
pub struct AddonVersion {
    pub id: String,
    pub name: Option<String>,
    pub environment: Option<VersionEnvironment>,
}

/// Serializes as `"pending"`, `"installed"`, or `{"failed": {"error": ...}}`. Mirrors
/// Archon.Content.v1.AddonStatus.
#[allow(dead_code)] // Adds answer once placed or with their error, so none is pending or failed.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AddonStatus {
    Pending,
    Installed,
    Failed { error: String },
}

/// One file in the content list. Response of GET `.../addons/update`, and an item of GET
/// `.../addons`. Mirrors Archon.Content.v1.Addon.
#[derive(Debug, Clone, Serialize)]
pub struct Addon {
    pub id: String,
    pub filename: String,
    pub filesize: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub btime: Option<DateTime<Utc>>,
    pub disabled: bool,
    /// Off for the server alone; see `content_mutation::set_side`.
    pub disabled_server: bool,
    pub disabled_player: bool,
    pub side_toggle_unlocked: bool,
    pub kind: ContentType,
    pub from_modpack: bool,
    pub status: AddonStatus,
    pub pack_client_retained: bool,
    pub pack_client_depends: bool,
    /// Core does not guess sides when it installs a pack, so there is nothing to review.
    pub needs_side_type_review: bool,
    pub has_update: Option<String>,
    pub name: Option<String>,
    pub project_id: Option<String>,
    pub version: Option<AddonVersion>,
    pub owner: Option<ContentOwner>,
    pub icon_url: Option<String>,
    /// What the jar says about itself, for mods and plugins.
    pub manifest: Option<bedrock::upstream::jar::Manifest>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<bool>,
}

/// Where a linked server is with the newest version pushed to it. Mirrors
/// Archon.Content.v1.LinkedPackState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LinkedPackState {
    UpToDate,
    Downloading,
    Pending,
    Applying,
    Failed,
}

/// What a world is installing. Mirrors the `installing` union of Archon.Content.v1.Addons.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Installing {
    Loader,
    Modpack,
}

/// Which pack a world follows. Mirrors Archon.Content.v1.ModpackSpec.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "platform", rename_all = "snake_case")]
pub enum ModpackSpec {
    /// Mirrors Archon.Content.v1.ModpackSpecModrinth.
    Modrinth {
        project_id: String,
        version_id: String,
    },
    /// Mirrors Archon.Content.v1.ModpackSpecLocalFile.
    LocalFile {
        filename: String,
        name: String,
        description: Option<String>,
    },
}

/// The linked pack, as Modrinth's modpack card shows it. Response of GET
/// `.../content/modpack/update`, and the `modpack` of GET `.../addons`. Mirrors
/// Archon.Content.v1.ModpackFields.
#[derive(Debug, Clone, Serialize)]
pub struct ModpackFields {
    pub spec: ModpackSpec,
    pub has_update: Option<String>,
    pub title: Option<String>,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub owner: Option<ContentOwner>,
    pub version_number: Option<String>,
    pub date_published: Option<DateTime<Utc>>,
    pub downloads: Option<u64>,
    pub followers: Option<u64>,
    pub instance_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub state: Option<LinkedPackState>,
    pub error: Option<String>,
}

/// A file of a pack the owner edited since the pack placed it, or deleted. Item of
/// `GET /servers/{id}/content/edited`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct EditedFile {
    /// Relative to the server folder, with forward slashes.
    pub path: String,
    /// The owner deleted it (or disabled it) rather than changed it.
    pub deleted: bool,
    /// What the last pack update wants for it, waiting on a [`Decision`]; null when nothing waits.
    pub pending: Option<PendingChange>,
}

/// What a pack update wants for a file the owner edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum PendingChange {
    /// The new pack has another version of the file.
    Changed,
    /// The new pack no longer has the file.
    Removed,
}
