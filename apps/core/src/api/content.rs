//! A world's content: `POST .../content` (install), `.../content/repair`,
//! `.../content/unlink-modpack`, `.../content/modpack/update` (GET and POST), the
//! `.../addons` routes (list, add, install-many, delete, delete-many, disable, enable,
//! disable-many, enable-many, update, update-many), all under `/v1/servers/{id}/worlds/{wid}`, and
//! the uploads `POST /servers/{id}/v1/worlds/{wid}/content/upload-addon-file` and
//! `.../upload-modpack-file`. Mirrors `Archon.Content.v1` (`archon/content/v1.ts`) and
//! `kyros/content/v1.ts`.

#![allow(dead_code)] // Wired in step 4: content routes.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::properties::PropertiesFields;

/// Mirrors Archon.Content.v1.AddonKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddonKind {
    Mod,
    Plugin,
    Datapack,
    Shader,
    Resourcepack,
}

/// Mirrors Archon.Content.v1.ContentOwnerType.
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

/// Where a version runs. Mirrors Labrinth.Projects.v3.Environment.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    ClientAndServer,
    ClientOnly,
    ClientOnlyServerOptional,
    SingleplayerOnly,
    ServerOnly,
    ServerOnlyClientOptional,
    DedicatedServerOnly,
    ClientOrServer,
    ClientOrServerPrefersBoth,
    Unknown,
}

/// Mirrors Archon.Content.v1.AddonVersion.
#[derive(Debug, Clone, Serialize)]
pub struct AddonVersion {
    pub id: String,
    pub name: Option<String>,
    pub environment: Option<Environment>,
}

/// Serializes as `"pending"`, `"installed"`, or `{"failed": {"error": ...}}`. Mirrors
/// Archon.Content.v1.AddonStatus.
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
    pub kind: AddonKind,
    pub from_modpack: bool,
    pub status: AddonStatus,
    pub pack_client_retained: bool,
    pub pack_client_depends: bool,
    pub has_update: Option<String>,
    pub name: Option<String>,
    pub project_id: Option<String>,
    pub version: Option<AddonVersion>,
    pub owner: Option<ContentOwner>,
    pub icon_url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installed: Option<bool>,
}

/// Where a linked server is with the newest version pushed to it. Mirrors
/// Archon.Content.v1.LinkedPackState.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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

/// Why the last install failed. Mirrors the `error` object of Archon.Content.v1.Addons.
#[derive(Debug, Clone, Serialize)]
pub struct ContentError {
    pub message: String,
}

/// Response of GET `.../addons`. `addons` is null when the query asked for no addons. Mirrors
/// Archon.Content.v1.Addons.
#[derive(Debug, Clone, Serialize)]
pub struct Addons {
    pub modloader: Option<String>,
    pub modloader_version: Option<String>,
    pub game_version: Option<String>,
    pub modpack: Option<ModpackFields>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installing: Option<Installing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ContentError>,
    pub addons: Option<Vec<Addon>>,
}

/// Query of GET `.../addons`. Mirrors the `options` of `ArchonContentV1Module.getAddons`.
#[derive(Debug, Clone, Deserialize)]
pub struct AddonsQuery {
    #[serde(default)]
    pub from_modpack: Option<bool>,
    #[serde(default)]
    pub disabled: Option<bool>,
    #[serde(default)]
    pub addons: Option<bool>,
    #[serde(default)]
    pub updates: Option<bool>,
}

/// Body of POST `.../addons`, and an item of the array body of POST `.../addons/install-many`.
/// Mirrors Archon.Content.v1.AddAddonRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct AddAddonRequest {
    pub project_id: String,
    #[serde(default)]
    pub version_id: Option<String>,
    #[serde(default)]
    pub kind: Option<AddonKind>,
}

/// Body of POST `.../addons/delete`, `.../disable`, and `.../enable`. Mirrors
/// Archon.Content.v1.RemoveAddonRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct RemoveAddonRequest {
    pub kind: AddonKind,
    pub filename: String,
}

/// Body of POST `.../addons/delete-many`, `.../disable-many`, and `.../enable-many`. Mirrors the
/// `{ items }` body of `ArchonContentV1Module.deleteAddons`.
#[derive(Debug, Clone, Deserialize)]
pub struct RemoveAddonsRequest {
    pub items: Vec<RemoveAddonRequest>,
}

/// Query of GET `.../addons/update`. Mirrors the `filename` param of
/// `ArchonContentV1Module.getAddonUpdate`.
#[derive(Debug, Clone, Deserialize)]
pub struct AddonUpdateQuery {
    pub filename: String,
}

/// Body of POST `.../addons/update`, and an item of POST `.../addons/update-many`. No
/// `version_id` means the newest version. Mirrors Archon.Content.v1.UpdateAddonRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAddonRequest {
    pub filename: String,
    #[serde(default)]
    pub version_id: Option<String>,
}

/// Body of POST `.../addons/update-many`. Mirrors the `{ addons }` body of
/// `ArchonContentV1Module.updateAddons`.
#[derive(Debug, Clone, Deserialize)]
pub struct UpdateAddonsRequest {
    pub addons: Vec<UpdateAddonRequest>,
}

/// Mirrors Archon.Content.v1.Modloader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Modloader {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
    Paper,
    Purpur,
    Vanilla,
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

/// Body of POST `.../content`: install a pack or a bare loader. Mirrors
/// Archon.Content.v1.InstallWorldContent.
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "content_variant", rename_all = "snake_case")]
pub enum InstallWorldContent {
    Modpack {
        spec: ModpackSpec,
        soft_override: bool,
        #[serde(default)]
        properties: Option<PropertiesFields>,
    },
    Bare {
        loader: Modloader,
        version: String,
        #[serde(default)]
        game_version: Option<String>,
        soft_override: bool,
        #[serde(default)]
        properties: Option<PropertiesFields>,
    },
}

/// Query of POST `.../content/upload-modpack-file`. Mirrors the `soft_override` param of
/// `KyrosContentV1Module.uploadModpackFile` and `ArchonContentV1Module.uploadInstance`.
#[derive(Debug, Clone, Deserialize)]
pub struct ModpackUploadQuery {
    #[serde(default)]
    pub soft_override: bool,
}

/// The JSON `instance` part of the multipart body of POST `.../content/upload-modpack-file`, sent
/// when the pack is an App instance. Its `properties` part is a `PropertiesFields`. Mirrors the
/// `instance` argument of `ArchonContentV1Module.uploadInstance`.
#[derive(Debug, Clone, Deserialize)]
pub struct UploadedInstance {
    pub id: String,
    pub name: String,
}
