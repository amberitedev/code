//! A world's content: `POST .../content` (install), `.../content/repair`,
//! `.../content/unlink-modpack`, `.../content/modpack/update` (GET and POST), the
//! `.../addons` routes (list, add, install-many, delete, delete-many, disable, enable,
//! disable-many, enable-many, update, update-many), all under `/v1/servers/{id}/worlds/{wid}`, and
//! the uploads `POST /servers/{id}/v1/worlds/{wid}/content/upload-addon-file` and
//! `.../upload-modpack-file`. Mirrors `Archon.Content.v1` (`archon/content/v1.ts`) and
//! `kyros/content/v1.ts`. A server has one world, whose id is the server's.
//!
//! Also Amberite's `GET|POST /servers/{id}/content/edited`: the pack files the owner edited, and
//! their answer for those a pack update left alone.
//!
//! Viewers see the content list; changing it is for editors. Loader and pack installs answer at
//! once and go on in the background (see `content`).

use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::{
    Json,
    extract::{Multipart, Path, Query, State, multipart::Field},
    http::StatusCode,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use ts_rs::TS;

use super::{
    AppState,
    activity::Action,
    error::{ApiError, ErrorCode},
    properties::PropertiesFields,
};
use crate::{
    access::{Caller, Level, activity},
    content::{
        self, Busy,
        addons::{self, Listed},
        install::{self as content_install, Linked, PackSource},
        pack,
        records::{self, ContentFile, Origin, Pack, PendingPack, SideToggle},
        shared::{self, SharedVersion},
    },
    installs::{ModLoader, shared::SharedDirs},
    servers::{Hold, ServerError, records as servers_records},
};

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

/// Where a version runs. Mirrors Labrinth.Projects.v3.Environment.
#[allow(dead_code)] // Core sends no environments yet.
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
    /// Off for the server alone; see `addons::set_side`.
    pub disabled_server: bool,
    pub disabled_player: bool,
    pub side_toggle_unlocked: bool,
    pub kind: AddonKind,
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
    /// Always 0, as `Addon::needs_side_type_review` is always false.
    pub files_need_side_type_review: u32,
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
    pub needs_side_type_review: Option<bool>,
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

/// Body of POST `.../addons/set-enabled-server` and `.../set-enabled-player`. Mirrors
/// Archon.Content.v1.SetAddonEnabledRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct SetAddonEnabledRequest {
    pub kind: AddonKind,
    pub filename: String,
    pub enabled: bool,
}

/// Body of POST `.../addons/set-locked-side-toggle`. Mirrors
/// Archon.Content.v1.SetAddonSideToggleLockedRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct SetAddonSideToggleLockedRequest {
    pub kind: AddonKind,
    pub filename: String,
    pub locked: bool,
}

/// Body of POST `.../addons/complete-side-type-review`. Mirrors
/// Archon.Content.v1.CompleteSideTypeReviewRequest. Core has nothing to review, so it is only
/// checked.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct CompleteSideTypeReviewRequest {
    #[allow(dead_code)]
    #[serde(default)]
    pub addons: Option<Vec<RemoveAddonRequest>>,
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
        #[allow(dead_code)]
        // Core never erases, so a soft override is the only kind.
        soft_override: bool,
        #[serde(default)]
        properties: Option<PropertiesFields>,
        /// Amberite: install a pack for an older Minecraft than the server's world anyway.
        #[serde(default)]
        force: bool,
    },
    Bare {
        loader: Modloader,
        version: String,
        #[serde(default)]
        game_version: Option<String>,
        #[allow(dead_code)] // As above.
        soft_override: bool,
        #[serde(default)]
        properties: Option<PropertiesFields>,
    },
    /// Amberite: apply a version of an App instance's share (`content::shared`). On a running
    /// server it waits for the server to stop, or with `restart` stops it, applies, and starts
    /// it again.
    Instance {
        instance: UploadedInstance,
        share: SharedVersion,
        #[serde(default)]
        properties: Option<PropertiesFields>,
        #[serde(default)]
        restart: bool,
        #[serde(default)]
        force: bool,
    },
}

/// Query of POST `.../content/upload-modpack-file`. Mirrors the `soft_override` param of
/// `KyrosContentV1Module.uploadModpackFile` and `ArchonContentV1Module.uploadModpackFile`.
#[derive(Debug, Clone, Deserialize)]
pub struct ModpackUploadQuery {
    #[allow(dead_code)] // As above.
    #[serde(default)]
    pub soft_override: bool,
    /// Amberite: as `force` in `InstallWorldContent::Modpack`.
    #[serde(default)]
    pub force: bool,
}

/// Optional body of POST `.../content/modpack/update`. Amberite-only.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct UpdateModpack {
    /// Update even when the newest version is for an older Minecraft than the server's world.
    #[serde(default)]
    pub force: bool,
}

/// The App instance of an `instance` install. Mirrors the `instance` of
/// Archon.Content.v1.InstallWorldContent.
#[derive(Debug, Clone, Deserialize)]
pub struct UploadedInstance {
    pub id: String,
    pub name: String,
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

/// Response of `GET /servers/{id}/content/edited`.
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct EditedFiles {
    pub files: Vec<EditedFile>,
}

/// Body of `POST /servers/{id}/content/edited`: keep the owner's version of a pending file, or
/// replace it with the pack's (for a removed file, delete it).
#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Decision {
    pub path: String,
    pub choice: Choice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Choice {
    Keep,
    Replace,
}

impl From<content::Error> for ApiError {
    fn from(error: content::Error) -> Self {
        use content::Error;
        match error {
            Error::Server(error) => error.into(),
            error => {
                let code = match &error {
                    Error::Busy => ErrorCode::Conflict,
                    Error::NotFound(_) => ErrorCode::NotFound,
                    Error::Invalid(_)
                    | Error::Path(_)
                    | Error::Pack(_)
                    | Error::Archive(_) => ErrorCode::InvalidRequest,
                    _ => ErrorCode::Internal,
                };
                Self::new(code, format!("{error:#}"))
            }
        }
    }
}

impl From<std::io::Error> for ApiError {
    fn from(error: std::io::Error) -> Self {
        content::Error::Io(error).into()
    }
}

/// The one world of a server has the server's id.
pub fn world(id: &str, wid: &str) -> Result<(), ApiError> {
    if id.eq_ignore_ascii_case(wid) {
        Ok(())
    } else {
        Err(ApiError::new(
            ErrorCode::NotFound,
            format!("There is no world {wid}."),
        ))
    }
}

/// Take a server for a background install: its content, then the server itself.
fn begin(state: &AppState, id: &str) -> Result<(Hold, Busy), ApiError> {
    let busy = state.content.busy(id)?;
    let hold = state.servers.hold(id).map_err(|error| match error {
        ServerError::AlreadyRunning => ApiError::new(
            ErrorCode::Conflict,
            "Stop the server before changing its loader or pack.",
        ),
        error => error.into(),
    })?;
    Ok((hold, busy))
}

fn loader_of(loader: Modloader) -> Result<ModLoader, ApiError> {
    Ok(match loader {
        Modloader::Vanilla => ModLoader::Vanilla,
        Modloader::Fabric => ModLoader::Fabric,
        Modloader::Quilt => ModLoader::Quilt,
        Modloader::Forge => ModLoader::Forge,
        Modloader::NeoForge => ModLoader::NeoForge,
        Modloader::Paper => ModLoader::Paper,
        Modloader::Purpur => {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "Core does not run Purpur.",
            ));
        }
    })
}

/// The content list of a server, with what Modrinth knows about each file.
async fn addon_list(
    state: &AppState,
    id: &str,
    listed: Vec<Listed>,
    updates: bool,
) -> Result<Vec<Addon>, ApiError> {
    let rows = records::files(&state.db, id).await?;
    let rows: HashMap<String, ContentFile> = rows
        .into_iter()
        .map(|row| (row.path.clone(), row))
        .collect();
    let toggles: HashMap<String, SideToggle> = records::toggles(&state.db, id)
        .await?
        .into_iter()
        .map(|toggle| (toggle.path.clone(), toggle))
        .collect();
    let mut project_ids: Vec<String> = rows
        .values()
        .filter_map(|row| row.project_id.clone())
        .collect();
    project_ids.sort();
    project_ids.dedup();
    let version_ids: Vec<String> = rows
        .values()
        .filter_map(|row| row.version_id.clone())
        .collect();
    let modrinth = &state.content.modrinth;
    let (projects, versions) = tokio::join!(
        modrinth.projects(&project_ids),
        modrinth.versions_by_id(&version_ids)
    );
    let projects: HashMap<String, content::modrinth::Project> = projects
        .unwrap_or_default()
        .into_iter()
        .map(|project| (project.id.clone(), project))
        .collect();
    let versions: HashMap<String, content::modrinth::Version> = versions
        .unwrap_or_default()
        .into_iter()
        .map(|version| (version.id.clone(), version))
        .collect();
    let newer = if updates {
        let row_list: Vec<ContentFile> = rows.values().cloned().collect();
        addons::updates(&state.content, id, &listed, &row_list).await
    } else {
        HashMap::new()
    };
    Ok(listed
        .into_iter()
        .map(|item| {
            let row = rows.get(&item.path);
            let project = row
                .and_then(|row| row.project_id.as_ref())
                .and_then(|project| projects.get(project));
            let version = row
                .and_then(|row| row.version_id.as_ref())
                .and_then(|version| versions.get(version));
            let toggle = toggles.get(&item.path);
            let sides = addons::Sides::of(&item, toggle);
            Addon {
                id: item.path.clone(),
                filename: item.filename,
                filesize: item.size,
                btime: item.created,
                disabled: sides.disabled,
                disabled_server: sides.disabled_server,
                disabled_player: sides.disabled_player,
                side_toggle_unlocked: toggle
                    .is_some_and(|toggle| toggle.unlocked),
                kind: item.kind,
                from_modpack: row.is_some_and(|row| row.origin == Origin::Pack),
                status: AddonStatus::Installed,
                pack_client_retained: false,
                pack_client_depends: false,
                needs_side_type_review: false,
                has_update: newer
                    .get(&item.path)
                    .map(|version| version.id.clone()),
                name: project.map(|project| project.title.clone()),
                project_id: row.and_then(|row| row.project_id.clone()),
                version: version.map(|version| AddonVersion {
                    id: version.id.clone(),
                    name: Some(version.version_number.clone()),
                    environment: None,
                }),
                owner: None,
                icon_url: project.and_then(|project| project.icon_url.clone()),
                installed: None,
            }
        })
        .collect())
}

/// The pack a server follows, as the modpack card shows it.
async fn modpack_fields(
    state: &AppState,
    id: &str,
    check: bool,
) -> Result<Option<ModpackFields>, ApiError> {
    let Some(pack) = records::pack(&state.db, id).await? else {
        return Ok(None);
    };
    let modrinth = &state.content.modrinth;
    let (project, version, has_update) =
        match (&pack.project_id, &pack.version_id) {
            (Some(project_id), Some(version_id)) => {
                let (project, version) = tokio::join!(
                    modrinth.project(project_id),
                    modrinth.version(version_id)
                );
                let has_update = if check {
                    modrinth
                        .versions(project_id, &[], None)
                        .await
                        .ok()
                        .and_then(|versions| versions.into_iter().next())
                        .map(|latest| latest.id)
                        .filter(|latest| latest != version_id)
                } else {
                    None
                };
                (project.ok(), version.ok(), has_update)
            }
            _ => (None, None, None),
        };
    let linked_state = linked_state(state, id, &pack).await?;
    let spec = match (&pack.project_id, &pack.version_id) {
        (Some(project_id), Some(version_id)) => ModpackSpec::Modrinth {
            project_id: project_id.clone(),
            version_id: version_id.clone(),
        },
        _ => ModpackSpec::LocalFile {
            filename: format!("{}.mrpack", pack.name),
            name: pack.name.clone(),
            description: pack.summary.clone(),
        },
    };
    Ok(Some(ModpackFields {
        spec,
        has_update,
        title: Some(
            project
                .as_ref()
                .map_or(pack.name.clone(), |project| project.title.clone()),
        ),
        description: project
            .as_ref()
            .and_then(|project| project.description.clone())
            .or(pack.summary.clone()),
        icon_url: project
            .as_ref()
            .and_then(|project| project.icon_url.clone()),
        owner: None,
        version_number: Some(
            version
                .as_ref()
                .map_or(pack.version_number.clone(), |version| {
                    version.version_number.clone()
                }),
        ),
        date_published: version
            .as_ref()
            .and_then(|version| version.date_published),
        downloads: project.as_ref().and_then(|project| project.downloads),
        followers: project.as_ref().and_then(|project| project.followers),
        instance_id: pack.instance_id.clone(),
        state: Some(linked_state),
        error: pack.error,
    }))
}

/// Where a server is with the pack it follows: applying it, failed, waiting for the server to
/// stop to apply a newer push, or up to date.
pub async fn linked_state(
    state: &AppState,
    id: &str,
    pack: &Pack,
) -> Result<LinkedPackState, ApiError> {
    Ok(
        if state.content.activity(id).installing == Some(Installing::Modpack) {
            LinkedPackState::Applying
        } else if pack.error.is_some() {
            LinkedPackState::Failed
        } else if records::pending(&state.db, id).await?.is_some() {
            LinkedPackState::Pending
        } else {
            LinkedPackState::UpToDate
        },
    )
}

/// `GET .../addons`.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<AddonsQuery>,
) -> Result<Json<Addons>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    let server = servers_records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let addons = if query.addons == Some(false) {
        None
    } else {
        let folder =
            content::folder::Folder::open(&state.servers.paths().server(&id))?;
        let listed: Vec<Listed> =
            addons::scan(&folder, loader).into_iter().collect();
        let mut addons =
            addon_list(&state, &id, listed, query.updates == Some(true))
                .await?;
        addons.retain(|addon| {
            query
                .from_modpack
                .is_none_or(|wanted| addon.from_modpack == wanted)
                && query.disabled.is_none_or(|wanted| addon.disabled == wanted)
                && query
                    .needs_side_type_review
                    .is_none_or(|wanted| addon.needs_side_type_review == wanted)
        });
        Some(addons)
    };
    let activity = state.content.activity(&id);
    let modpack =
        modpack_fields(&state, &id, query.updates == Some(true)).await?;
    let error = activity
        .error
        .clone()
        .or_else(|| modpack.as_ref().and_then(|pack| pack.error.clone()))
        .map(|message| ContentError { message });
    Ok(Json(Addons {
        modloader: server.loader,
        modloader_version: server.loader_version,
        game_version: server.game_version,
        modpack,
        installing: activity.installing,
        error,
        files_need_side_type_review: 0,
        addons,
    }))
}

/// `POST .../addons`: add a Modrinth project and the dependencies nothing provides yet.
pub async fn add(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<AddAddonRequest>,
) -> Result<StatusCode, ApiError> {
    add_many(caller, State(state), Path((id, wid)), Json(vec![request])).await
}

/// `POST .../addons/install-many`.
pub async fn add_many(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(requests): Json<Vec<AddAddonRequest>>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let _busy = state.content.busy(&id)?;
    let result = async {
        for request in &requests {
            addons::add(
                &state.content,
                &id,
                &request.project_id,
                request.version_id.as_deref(),
                request.kind,
            )
            .await?;
        }
        Ok::<_, content::Error>(())
    }
    .await;
    result?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// What `delete`, `disable`, and `enable` do to each file.
#[derive(Clone, Copy)]
enum Change {
    Delete,
    Enable(bool),
}

async fn change(
    state: &AppState,
    caller: &Caller,
    id: &str,
    wid: &str,
    items: &[RemoveAddonRequest],
    change: Change,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, id, Level::Editor).await?;
    world(id, wid)?;
    let _busy = state.content.busy(id)?;
    for item in items {
        match change {
            Change::Delete => {
                addons::remove(&state.content, id, item.kind, &item.filename)
                    .await?
            }
            Change::Enable(enabled) => {
                addons::set_enabled(
                    &state.content,
                    id,
                    item.kind,
                    &item.filename,
                    enabled,
                )
                .await?;
            }
        }
    }
    activity::log(&state.db, caller, Some(id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST .../addons/delete`.
pub async fn delete(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(item): Json<RemoveAddonRequest>,
) -> Result<StatusCode, ApiError> {
    change(&state, &caller, &id, &wid, &[item], Change::Delete).await
}

/// `POST .../addons/delete-many`.
pub async fn delete_many(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<RemoveAddonsRequest>,
) -> Result<StatusCode, ApiError> {
    change(&state, &caller, &id, &wid, &body.items, Change::Delete).await
}

/// `POST .../addons/disable`.
pub async fn disable(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(item): Json<RemoveAddonRequest>,
) -> Result<StatusCode, ApiError> {
    change(&state, &caller, &id, &wid, &[item], Change::Enable(false)).await
}

/// `POST .../addons/enable`.
pub async fn enable(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(item): Json<RemoveAddonRequest>,
) -> Result<StatusCode, ApiError> {
    change(&state, &caller, &id, &wid, &[item], Change::Enable(true)).await
}

/// `POST .../addons/disable-many`.
pub async fn disable_many(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<RemoveAddonsRequest>,
) -> Result<StatusCode, ApiError> {
    change(
        &state,
        &caller,
        &id,
        &wid,
        &body.items,
        Change::Enable(false),
    )
    .await
}

/// `POST .../addons/enable-many`.
pub async fn enable_many(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<RemoveAddonsRequest>,
) -> Result<StatusCode, ApiError> {
    change(
        &state,
        &caller,
        &id,
        &wid,
        &body.items,
        Change::Enable(true),
    )
    .await
}

/// Refuses a server that follows an instance. Its instance's owner writes the share it reads,
/// side toggles included, so it has no share or toggles of its own.
pub async fn server_first(state: &AppState, id: &str) -> Result<(), ApiError> {
    let linked = records::pack(&state.db, id)
        .await?
        .is_some_and(|pack| content_install::linked(&pack).is_some());
    if linked {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "This server follows an instance, which its owner shares.",
        ));
    }
    Ok(())
}

async fn set_side(
    state: &AppState,
    caller: &Caller,
    id: &str,
    wid: &str,
    request: SetAddonEnabledRequest,
    side: addons::Side,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, id, Level::Editor).await?;
    world(id, wid)?;
    server_first(state, id).await?;
    let _busy = state.content.busy(id)?;
    addons::set_side(
        &state.content,
        id,
        request.kind,
        &request.filename,
        side,
        request.enabled,
    )
    .await?;
    activity::log(&state.db, caller, Some(id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST .../addons/set-enabled-server`.
pub async fn set_enabled_server(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<SetAddonEnabledRequest>,
) -> Result<StatusCode, ApiError> {
    set_side(&state, &caller, &id, &wid, request, addons::Side::Server).await
}

/// `POST .../addons/set-enabled-player`.
pub async fn set_enabled_player(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<SetAddonEnabledRequest>,
) -> Result<StatusCode, ApiError> {
    set_side(&state, &caller, &id, &wid, request, addons::Side::Player).await
}

/// `POST .../addons/set-locked-side-toggle`.
pub async fn set_side_toggle_locked(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<SetAddonSideToggleLockedRequest>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    server_first(&state, &id).await?;
    let _busy = state.content.busy(&id)?;
    addons::set_unlocked(
        &state.content,
        &id,
        request.kind,
        &request.filename,
        !request.locked,
    )
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST .../addons/complete-side-type-review`: nothing to do, since Core never asks for one.
pub async fn complete_side_type_review(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(_): Json<CompleteSideTypeReviewRequest>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET .../addons/update?filename`: the file, with its newest version in `has_update`.
pub async fn get_update(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<AddonUpdateQuery>,
) -> Result<Json<Addon>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    let server = servers_records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let folder =
        content::folder::Folder::open(&state.servers.paths().server(&id))?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let listed =
        addons::find(&folder, loader, &query.filename).ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                format!("There is no {}.", query.filename),
            )
        })?;
    let addon = addon_list(&state, &id, vec![listed], true)
        .await?
        .pop()
        .expect("one in, one out");
    Ok(Json(addon))
}

/// `POST .../addons/update`.
pub async fn update(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<UpdateAddonRequest>,
) -> Result<StatusCode, ApiError> {
    update_many(
        caller,
        State(state),
        Path((id, wid)),
        Json(UpdateAddonsRequest {
            addons: vec![request],
        }),
    )
    .await
}

/// `POST .../addons/update-many`.
pub async fn update_many(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<UpdateAddonsRequest>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let _busy = state.content.busy(&id)?;
    let result = async {
        for request in &body.addons {
            addons::update(
                &state.content,
                &id,
                &request.filename,
                request.version_id.as_deref(),
            )
            .await?;
        }
        Ok::<_, content::Error>(())
    }
    .await;
    result?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `POST .../content`: install a pack or a bare loader on a stopped server, in the background.
/// A server with something installed is copied first. A bare install stops following the pack;
/// its files stay. Nothing is ever erased, so `soft_override` changes nothing.
pub async fn install(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<InstallWorldContent>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let server = servers_records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    match request {
        InstallWorldContent::Bare {
            loader,
            version,
            game_version,
            properties,
            ..
        } => {
            let loader = loader_of(loader)?;
            let game_version = game_version
                .filter(|version| !version.is_empty())
                .or(server.game_version)
                .ok_or_else(|| {
                    ApiError::new(
                        ErrorCode::InvalidRequest,
                        "Choose a Minecraft version.",
                    )
                })?;
            let requested = Some(version).filter(|version| !version.is_empty());
            let (hold, busy) = begin(&state, &id)?;
            let access = state.access.clone();
            state.content.spawn(
                hold,
                busy,
                Installing::Loader,
                move |content, hold, progress| async move {
                    records::unlink(&content.db, hold.id()).await?;
                    access.owed.notify_one();
                    content_install::loader(
                        &content,
                        &hold,
                        &progress,
                        loader,
                        &game_version,
                        requested.as_deref(),
                        false,
                    )
                    .await?;
                    if let Some(properties) = properties {
                        content_install::write_properties(
                            &content.paths.server(hold.id()),
                            &properties,
                        )?;
                    }
                    Ok(None)
                },
            );
        }
        InstallWorldContent::Instance {
            instance,
            share,
            properties,
            restart,
            force,
        } => {
            install_instance(
                &state, &id, instance, share, properties, restart, force,
            )
            .await?;
        }
        InstallWorldContent::Modpack {
            spec,
            properties,
            force,
            ..
        } => {
            let ModpackSpec::Modrinth {
                project_id,
                version_id,
            } = spec
            else {
                return Err(ApiError::new(
                    ErrorCode::InvalidRequest,
                    "Send a pack file through upload-modpack-file.",
                ));
            };
            let (hold, busy) = begin(&state, &id)?;
            spawn_pack(
                &state,
                hold,
                busy,
                PackSource::Modrinth {
                    project_id,
                    version_id,
                },
                properties,
                force,
            );
        }
    }
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::ACCEPTED)
}

fn spawn_pack(
    state: &AppState,
    hold: Hold,
    busy: Busy,
    source: PackSource,
    properties: Option<PropertiesFields>,
    force: bool,
) {
    state.content.spawn(
        hold,
        busy,
        Installing::Modpack,
        move |content, hold, progress| async move {
            content_install::pack(
                &content, &hold, &progress, source, properties, force,
            )
            .await
        },
    );
}

/// Apply a share version to a server. A stopped server installs it now. A running one keeps
/// running: Core fetches the version, then it waits for the server to stop, or with `restart`
/// Core stops the server, and it applies then (see [`apply_pending`]).
async fn install_instance(
    state: &Arc<AppState>,
    id: &str,
    instance: UploadedInstance,
    share: SharedVersion,
    properties: Option<PropertiesFields>,
    restart: bool,
    force: bool,
) -> Result<(), ApiError> {
    let busy = state.content.busy(id)?;
    let linked = Linked {
        id: instance.id,
        name: instance.name,
        shared_instance_id: share.shared_instance_id.clone(),
        version: share.version.into(),
    };
    match state.servers.hold(id) {
        Ok(hold) => {
            // A version still waiting from an earlier push is superseded by this one.
            let waiting = records::clear_pending(&state.db, id).await?;
            forget_pack(state, waiting).await;
            let app = Arc::clone(state);
            state.content.spawn(
                hold,
                busy,
                Installing::Modpack,
                move |content, hold, progress| async move {
                    let sha1 =
                        match shared::write_pack(&content, hold.id(), &share)
                            .await
                        {
                            Ok(sha1) => sha1,
                            Err(error) => {
                                records::set_pack_error(
                                    &content.db,
                                    hold.id(),
                                    Some(&format!("{error:#}")),
                                )
                                .await?;
                                return Err(error);
                            }
                        };
                    let source = PackSource::Stored {
                        sha1,
                        instance: Some(linked),
                    };
                    let change = content_install::pack(
                        &content, &hold, &progress, source, properties, force,
                    )
                    .await?;
                    drop_own_share(&app, hold.id()).await?;
                    Ok(change)
                },
            );
        }
        Err(ServerError::AlreadyRunning) => {
            let state = Arc::clone(state);
            let id = id.to_owned();
            tokio::spawn(async move {
                let written =
                    shared::write_pack(&state.content, &id, &share).await;
                drop(busy);
                let result = async {
                    let pending = PendingPack {
                        sha1: written?,
                        instance_id: linked.id,
                        instance_name: linked.name,
                        shared_instance_id: linked.shared_instance_id,
                        shared_version: linked.version,
                        start_after: restart,
                        force,
                    };
                    let replaced =
                        records::set_pending(&state.db, &id, &pending).await?;
                    forget_pack(&state, replaced).await;
                    drop_own_share(&state, &id).await?;
                    if restart {
                        match state.servers.stop(&id).await {
                            // It stopped on its own while the version was fetched.
                            Err(ServerError::NotRunning) => {}
                            result => result?,
                        }
                    }
                    Ok::<_, content::Error>(())
                }
                .await;
                match result {
                    // The server may have stopped while the version was fetched.
                    Ok(()) => apply_pending(state, id).await,
                    Err(error) => {
                        tracing::warn!(server = %id, %error, "could not take a pushed version");
                        let _ = records::set_pack_error(
                            &state.db,
                            &id,
                            Some(&format!("{error:#}")),
                        )
                        .await;
                    }
                }
            });
        }
        Err(error) => return Err(error.into()),
    }
    Ok(())
}

/// Let go of the server's own share once it follows an instance, since a server never has two
/// shares. Called only after the link is recorded, so a failed link keeps the share and its
/// players.
async fn drop_own_share(state: &AppState, id: &str) -> sqlx::Result<()> {
    let mut tx = state.db.begin().await?;
    super::share::owe_share_deletion(&mut tx, id).await?;
    tx.commit().await?;
    state.access.owed.notify_one();
    Ok(())
}

/// Apply the version waiting for a server, when one waits and the server is stopped. Starts the
/// server again afterwards when the push asked for a restart.
/// Another change to the server's content is waited for, so it never costs the version.
pub async fn apply_pending(state: Arc<AppState>, id: String) {
    let id = id.as_str();
    let busy = state.content.busy_wait(id).await;
    let Ok(Some(pending)) = records::pending(&state.db, id).await else {
        return;
    };
    let Ok(hold) = state.servers.hold(id) else {
        return;
    };
    if records::clear_pending(&state.db, id).await.is_err() {
        return;
    }
    let (start_after, force) = (pending.start_after, pending.force);
    let source = PackSource::Stored {
        sha1: pending.sha1,
        instance: Some(Linked {
            id: pending.instance_id,
            name: pending.instance_name,
            shared_instance_id: pending.shared_instance_id,
            version: pending.shared_version,
        }),
    };
    state.content.spawn(
        hold,
        busy,
        Installing::Modpack,
        move |content, hold, progress| async move {
            let result = content_install::pack(
                &content, &hold, &progress, source, None, force,
            )
            .await;
            if start_after {
                // Back up either way: a failed update changes nothing it could not finish.
                let id = hold.id().to_owned();
                drop(hold);
                if let Err(error) = content.servers.start(&id).await {
                    tracing::warn!(server = %id, %error, "could not start the server after its update");
                }
            }
            result
        },
    );
}

/// Delete a pack file nothing uses any more, such as a waiting version a newer push replaced.
async fn forget_pack(state: &AppState, sha1: Option<String>) {
    let Some(sha1) = sha1 else { return };
    if let Ok(false) = records::pack_in_use(&state.db, &sha1).await {
        let dirs = SharedDirs::new(state.servers.paths().root());
        let _ = tokio::fs::remove_file(dirs.pack(&sha1)).await;
    }
}

/// Apply pushed versions as their servers stop, for as long as Core runs.
pub async fn apply_pending_on_stop(state: Arc<AppState>) {
    use tokio::sync::broadcast::error::RecvError;

    let mut events = state.servers.events.subscribe();
    // Versions pushed before Core last stopped, to servers that are stopped now.
    for id in records::servers_pending(&state.db)
        .await
        .unwrap_or_default()
    {
        tokio::spawn(apply_pending(Arc::clone(&state), id));
    }
    loop {
        match events.recv().await {
            Ok(crate::servers::Lifecycle::Ended(id)) => {
                tokio::spawn(apply_pending(Arc::clone(&state), id));
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}

/// `POST .../content/repair`: check every file of the loader and the pack again, put back pack
/// files that are missing, and clear a failed install. Edited files stay as they are, and nothing
/// a running server uses is removed.
pub async fn repair(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let server = servers_records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let (hold, busy) = begin(&state, &id)?;
    if records::pack(&state.db, &id).await?.is_some() {
        // The pack the server already follows: no other Minecraft.
        spawn_pack(&state, hold, busy, PackSource::Current, None, true);
    } else {
        let (Some(loader), Some(game_version)) = (
            server.loader.as_deref().and_then(ModLoader::parse),
            server.game_version,
        ) else {
            return Err(ApiError::new(
                ErrorCode::Conflict,
                "Nothing is installed on this server yet.",
            ));
        };
        let requested = server.loader_version;
        state.content.spawn(
            hold,
            busy,
            Installing::Loader,
            move |content, hold, progress| async move {
                content_install::loader(
                    &content,
                    &hold,
                    &progress,
                    loader,
                    &game_version,
                    requested.as_deref(),
                    true,
                )
                .await
                .map(|()| None)
            },
        );
    }
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::ACCEPTED)
}

/// `POST .../content/unlink-modpack`: stop following the pack. Its files stay, as added files,
/// and a failed pack install is cleared.
pub async fn unlink(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let _busy = state.content.busy(&id)?;
    records::unlink(&state.db, &id).await?;
    state.access.owed.notify_one();
    state.content.clear_error(&id);
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET .../content/modpack/update`: the pack, with its newest version in `has_update`.
pub async fn modpack(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<ModpackFields>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    modpack_fields(&state, &id, true)
        .await?
        .map(Json)
        .ok_or_else(|| {
            ApiError::new(ErrorCode::NotFound, "This server follows no pack.")
        })
}

/// `POST .../content/modpack/update`: update a Modrinth pack to its newest version, in the
/// background. Files the owner edited are left alone (see `/servers/{id}/content/edited`). A
/// newest version for an older Minecraft is refused unless the body says `force`.
///
/// The body is read by hand: the App's client sends `Content-Type: application/json` on every
/// request, with no body here, which axum's optional `Json` would refuse.
pub async fn update_modpack(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    body: bytes::Bytes,
) -> Result<StatusCode, ApiError> {
    let force = if body.is_empty() {
        false
    } else {
        serde_json::from_slice::<UpdateModpack>(&body)
            .map_err(|error| {
                ApiError::new(ErrorCode::InvalidRequest, error.to_string())
            })?
            .force
    };
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let pack = records::pack(&state.db, &id).await?.ok_or_else(|| {
        ApiError::new(ErrorCode::NotFound, "This server follows no pack.")
    })?;
    let Some(project_id) = pack.project_id else {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "This pack came from a file; upload the new file instead.",
        ));
    };
    let latest = state
        .content
        .modrinth
        .versions(&project_id, &[], None)
        .await
        .map_err(content::Error::from)?
        .into_iter()
        .next()
        .ok_or_else(|| {
            ApiError::new(ErrorCode::NotFound, "The pack has no versions.")
        })?;
    let (hold, busy) = begin(&state, &id)?;
    spawn_pack(
        &state,
        hold,
        busy,
        PackSource::Modrinth {
            project_id,
            version_id: latest.id,
        },
        None,
        force,
    );
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::ACCEPTED)
}

/// The most one upload request may write.
pub const MAX_UPLOAD: u64 = 4 * 1024 * 1024 * 1024;

/// Stream one multipart field into `path`, at most `limit` bytes, returning its SHA-1.
pub async fn stage_field(
    field: &mut Field<'_>,
    path: &std::path::Path,
    limit: u64,
) -> Result<String, ApiError> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let result = async {
        let mut file = tokio::fs::File::create(path).await?;
        let mut hasher = sha1::Sha1::default();
        let mut written = 0u64;
        while let Some(chunk) = field.chunk().await.map_err(|error| {
            ApiError::new(ErrorCode::InvalidRequest, error.body_text())
        })? {
            written += chunk.len() as u64;
            if written > limit {
                return Err(ApiError::new(
                    ErrorCode::InvalidRequest,
                    format!(
                        "Uploads are at most {} GB.",
                        limit / 1024 / 1024 / 1024
                    ),
                ));
            }
            sha1::Digest::update(&mut hasher, &chunk);
            file.write_all(&chunk).await?;
        }
        file.flush().await?;
        Ok(hex::encode(sha1::Digest::finalize(hasher)))
    }
    .await;
    // Whatever stopped it, a client gone or a full disk, nothing half-written stays.
    if result.is_err() {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}

fn bad_form(error: axum::extract::multipart::MultipartError) -> ApiError {
    ApiError::new(ErrorCode::InvalidRequest, error.body_text())
}

/// `POST /servers/{id}/v1/worlds/{wid}/content/upload-modpack-file`: install an uploaded
/// `.mrpack` in the background. Parts: `file` and `properties` (JSON settings).
pub async fn upload_modpack(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<ModpackUploadQuery>,
    mut form: Multipart,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let (hold, busy) = begin(&state, &id)?;
    let dirs = SharedDirs::new(state.servers.paths().root());
    let mut staged: Option<(PathBuf, String)> = None;
    let mut properties: Option<PropertiesFields> = None;
    let result = async {
        while let Some(mut field) = form.next_field().await.map_err(bad_form)? {
            match field.name() {
                Some("file") => {
                    let path =
                        dirs.pack(&format!("upload-{}", uuid::Uuid::new_v4()));
                    let sha1 = stage_field(
                        &mut field,
                        &path,
                        content_install::MAX_PACK,
                    )
                    .await?;
                    staged = Some((path, sha1));
                }
                Some("properties") => {
                    let text = field.text().await.map_err(bad_form)?;
                    properties = serde_json::from_str(&text).ok();
                }
                _ => {}
            }
        }
        let (path, sha1) = staged.take().ok_or_else(|| {
            ApiError::new(
                ErrorCode::InvalidRequest,
                "The upload has no pack file.",
            )
        })?;
        let stored = dirs.pack(&sha1);
        tokio::fs::rename(&path, &stored).await?;
        // A file that is not a pack is refused now, not in the background. Spellings that
        // collide are checked at install, against the server's own disk.
        content_install::read_pack(&stored, false)
            .await
            .map_err(|error| {
                let stored = stored.clone();
                tokio::spawn(async move {
                    let _ = tokio::fs::remove_file(stored).await;
                });
                ApiError::from(error)
            })?;
        Ok::<_, ApiError>(sha1)
    }
    .await;
    if let Some((path, _)) = staged {
        let _ = tokio::fs::remove_file(path).await;
    }
    let sha1 = result?;
    spawn_pack(
        &state,
        hold,
        busy,
        PackSource::Stored {
            sha1,
            instance: None,
        },
        properties,
        query.force,
    );
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::ACCEPTED)
}

/// `POST /servers/{id}/v1/worlds/{wid}/content/upload-addon-file`: place uploaded `.jar` mods or
/// plugins and `.zip` datapacks, one `file` part each.
pub async fn upload_addons(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    mut form: Multipart,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let _busy = state.content.busy(&id)?;
    let staging = state
        .servers
        .paths()
        .staging(&id)
        .join(uuid::Uuid::new_v4().to_string());
    let result = async {
        let mut budget = MAX_UPLOAD;
        let mut n = 0;
        while let Some(mut field) = form.next_field().await.map_err(bad_form)? {
            let Some(filename) = field.file_name().map(str::to_owned) else {
                continue;
            };
            n += 1;
            let path = staging.join(n.to_string());
            stage_field(&mut field, &path, budget).await?;
            budget =
                budget.saturating_sub(tokio::fs::metadata(&path).await?.len());
            addons::upload(&state.content, &id, &filename, &path).await?;
        }
        Ok::<_, ApiError>(())
    }
    .await;
    let _ = tokio::fs::remove_dir_all(&staging).await;
    result?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `GET /servers/{id}/content/edited`: the pack files the owner changed or deleted since the
/// pack placed them, with what a pack update wants for each.
pub async fn edited(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<EditedFiles>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    let rows = records::files(&state.db, &id).await?;
    let folder = Arc::new(content::folder::Folder::open(
        &state.servers.paths().server(&id),
    )?);
    let files = tokio::task::spawn_blocking(move || {
        pack::edited(&rows, &|path| folder.sha1(path).ok().flatten())
            .into_iter()
            .map(|(row, on_disk)| EditedFile {
                path: row.path.clone(),
                deleted: on_disk.is_none(),
                pending: row.pending.map(|pending| match pending {
                    records::Pending::Changed => PendingChange::Changed,
                    records::Pending::Removed => PendingChange::Removed,
                }),
            })
            .collect()
    })
    .await
    .map_err(std::io::Error::other)?;
    Ok(Json(EditedFiles { files }))
}

/// `POST /servers/{id}/content/edited`: keep or replace one file a pack update left alone.
pub async fn decide(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(decision): Json<Decision>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let _busy = state.content.busy(&id)?;
    let path =
        content::folder::clean(&decision.path).map_err(content::Error::from)?;
    content_install::decide(
        &state.content,
        &id,
        &path,
        decision.choice == Choice::Replace,
    )
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}
