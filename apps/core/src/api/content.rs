//! A world's content: `POST .../content` (install), `.../content/repair`,
//! `.../content/unlink-modpack`, `.../content/modpack/update` (GET and POST), the
//! `.../addons` routes (list, add, install-many, delete, delete-many, disable, enable,
//! disable-many, enable-many, update, update-many), all under `/v1/servers/{id}/worlds/{wid}`, and
//! the uploads `POST /servers/{id}/v1/worlds/{wid}/content/upload-addon-file` and
//! `.../upload-modpack-file`, and `GET /servers/{id}/v1/worlds/{wid}/content/embedded-icon`. Mirrors `Archon.Content.v1` (`archon/content/v1.ts`) and
//! `kyros/content/v1.ts`. A server has one world, whose id is the server's.
//!
//! Also Amberite's `GET|POST /servers/{id}/content/edited`: the pack files the owner edited, and
//! their answer for those a pack update left alone.
//!
//! Viewers see the content list; changing it is for editors. Loader and pack installs answer at
//! once and go on in the background (see `state::servers::Content`). The work is in
//! `state::servers::commands`; these routes check access and map requests and errors.
//!
//! Where Archon's names a concept the shared crates have, the shared type is on the wire, with
//! the same names: an addon's `kind` is `ContentType` (Archon's `AddonKind`, which has no
//! `modpack`), the update channel is `VersionType` (Archon's `ReleaseChannel`), and a version's
//! `environment` is bedrock's `VersionEnvironment`.

use std::{path::PathBuf, sync::Arc};

use axum::{
    Json,
    extract::{Multipart, Path, Query, State, multipart::Field},
    http::{StatusCode, header::CONTENT_TYPE},
};
use modrinth_content_management::{ContentType, VersionType};
use serde::{Deserialize, Serialize};
use sharing_client::consume;
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
    installs::{ModLoader, shared::SharedDirs},
    servers::{Hold, ServerError},
    state::servers::{
        self as content, Busy,
        adapters::{
            filesystem::{
                archive::{self, NoRoom},
                content_files::{self, Listed},
                folder,
            },
            sqlite::{content_rows, server_rows},
        },
        commands::{
            apply_content_install::{add_project, add_uploaded_file},
            apply_content_update::update_project,
            check_content_updates::check_modpack_update,
            content_mutation::{self, Side},
            edited_files::{decide_edited_file, list_edited_files},
            install_instance::{self, InstanceInstall},
            install_loader::install_loader,
            install_pack::{self as content_install, PackSource},
            list_content::{get_linked_modpack_info, list_content, open_jar},
        },
        modrinth_pack,
    },
};

// The content list and edited files are what `state::servers::commands` produce.
pub use crate::state::servers::model::{
    Addon, EditedFile, Installing, LinkedPackState, ModpackFields, ModpackSpec,
    PendingChange,
};

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
    /// The least stable channel addon updates are offered from.
    pub update_channel: VersionType,
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
    pub kind: Option<ContentType>,
}

/// Body of POST `.../addons/delete`, `.../disable`, and `.../enable`. Mirrors
/// Archon.Content.v1.RemoveAddonRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct RemoveAddonRequest {
    pub kind: ContentType,
    pub filename: String,
}

/// Body of POST `.../addons/set-enabled-server` and `.../set-enabled-player`. Mirrors
/// Archon.Content.v1.SetAddonEnabledRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct SetAddonEnabledRequest {
    pub kind: ContentType,
    pub filename: String,
    pub enabled: bool,
}

/// Body of POST `.../addons/set-locked-side-toggle`. Mirrors
/// Archon.Content.v1.SetAddonSideToggleLockedRequest.
#[derive(Debug, Clone, Deserialize)]
pub struct SetAddonSideToggleLockedRequest {
    pub kind: ContentType,
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
    /// Amberite: apply the latest version of an App instance's share (`commands::write_shared_pack`), which
    /// Core reads from the backend itself. On a running server it waits for the server to stop,
    /// or with `restart` stops it, applies, and starts it again.
    Instance {
        instance: UploadedInstance,
        shared_instance_id: String,
        /// The backend's read ticket for this Core and share, which the App gets with its user's
        /// session. Required from the App; the local CLI has none and reads as the owner.
        #[serde(default)]
        ticket: Option<String>,
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
                    Error::Resolve(
                        modrinth_content_management::Error::Provider(_),
                    ) => ErrorCode::Internal,
                    // The disk, not the request: the same one may go once there is room.
                    Error::NoRoom(_)
                    | Error::Archive(archive::ArchiveError::NoRoom(_))
                    | Error::Pack(content::domain::pack::PackError::NoRoom(
                        _,
                    )) => ErrorCode::Conflict,
                    Error::Invalid(_)
                    | Error::Resolve(_)
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

/// `GET .../addons`.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<AddonsQuery>,
) -> Result<Json<Addons>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    let server = server_rows::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let addons = if query.addons == Some(false) {
        None
    } else {
        let folder = folder::Folder::open(&state.servers.paths().server(&id))?;
        let listed: Vec<Listed> =
            content_files::scan(&folder, loader).into_iter().collect();
        let skipped = content_rows::get_skipped_files(&state.db, &id).await?;
        let mut addons = list_content(
            &state.content,
            &id,
            loader,
            listed,
            skipped,
            query.updates == Some(true),
        )
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
    let modpack = get_linked_modpack_info(
        &state.content,
        &id,
        query.updates == Some(true),
    )
    .await?;
    let error = activity
        .error
        .clone()
        .or_else(|| modpack.as_ref().and_then(|pack| pack.error.clone()))
        .map(|message| ContentError { message });
    Ok(Json(Addons {
        modloader: server.loader,
        modloader_version: server.loader_version,
        game_version: server.game_version,
        update_channel: server.update_channel,
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
            add_project(
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
                content_mutation::remove_project(
                    &state.content,
                    id,
                    item.kind,
                    &item.filename,
                )
                .await?
            }
            Change::Enable(enabled) => {
                content_mutation::set_enabled(
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
/// sides included, so it has no share or sides of its own.
pub async fn server_first(state: &AppState, id: &str) -> Result<(), ApiError> {
    let linked = content_rows::get_pack(&state.db, id)
        .await?
        .is_some_and(|pack| pack.share.is_some());
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
    side: Side,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, id, Level::Editor).await?;
    world(id, wid)?;
    server_first(state, id).await?;
    let _busy = state.content.busy(id)?;
    content_mutation::set_side(
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
    set_side(&state, &caller, &id, &wid, request, Side::Server).await
}

/// `POST .../addons/set-enabled-player`.
pub async fn set_enabled_player(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(request): Json<SetAddonEnabledRequest>,
) -> Result<StatusCode, ApiError> {
    set_side(&state, &caller, &id, &wid, request, Side::Player).await
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
    content_mutation::set_unlocked(
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
    let server = server_rows::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let folder = folder::Folder::open(&state.servers.paths().server(&id))?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let listed = content_files::find(&folder, loader, &query.filename)
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::NotFound,
                format!("There is no {}.", query.filename),
            )
        })?;
    let addon = list_content(
        &state.content,
        &id,
        loader,
        vec![listed],
        Vec::new(),
        true,
    )
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
            update_project(
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
    let server = server_rows::get(&state.db, &id)
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
                    content_rows::unlink(&content.db, hold.id()).await?;
                    access.owed.notify_one();
                    install_loader(
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
            shared_instance_id,
            ticket,
            properties,
            restart,
            force,
        } => {
            install_instance(
                &state,
                &id,
                &caller,
                ticket,
                instance,
                shared_instance_id,
                properties,
                restart,
                force,
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
    let access = state.access.clone();
    state.content.spawn(
        hold,
        busy,
        Installing::Modpack,
        move |content, hold, progress| async move {
            let result = content_install::pack(
                &content, &hold, &progress, source, properties, force,
            )
            .await;
            // A pack of its own may have dropped the players of the instance it followed.
            access.owed.notify_one();
            result
        },
    );
}

/// Apply the latest version of a share to a server (`commands::install_instance`), which Core
/// reads from the backend with its own key and the `ticket` of the user who asked, so it reads
/// only what that user can. Only the local CLI reads without one, as the Core's owner. The
/// version is read before this returns, so one that cannot go fails the request.
#[allow(clippy::too_many_arguments)]
async fn install_instance(
    state: &Arc<AppState>,
    id: &str,
    caller: &Caller,
    ticket: Option<String>,
    instance: UploadedInstance,
    shared_instance_id: String,
    properties: Option<PropertiesFields>,
    restart: bool,
    force: bool,
) -> Result<(), ApiError> {
    let busy = state.content.busy(id)?;
    let backend = super::share::backend(state).await?;
    let backend = match (ticket, caller) {
        (Some(ticket), _) => backend.with_read_ticket(ticket),
        (None, Caller::Local) => backend,
        // Without a ticket the backend reads as the Core's owner, which is not who asked.
        (None, Caller::Session { .. }) => {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "An instance install needs a read ticket for this Core.",
            ));
        }
    };
    let version = consume::ready_version(&backend, &shared_instance_id)
        .await
        .map_err(|error| match error {
            // The backend names why by its code. It checks the Core owner's account before the
            // reader's, so a locked or blacklisted account is most likely the owner's.
            consume::ReadError::Denied { code, description } => {
                let message = match code.as_str() {
                    "not_member" => {
                        "You no longer have access to this instance's share."
                    }
                    "ticket_invalid" => {
                        "The request to update the server expired. Try again."
                    }
                    "account_locked" | "blacklisted" => {
                        "The owner of this Core cannot use sharing right now."
                    }
                    _ => description.as_str(),
                };
                ApiError::new(ErrorCode::Forbidden, message)
            }
            consume::ReadError::Backend(error) => ApiError::from(error),
            error @ consume::ReadError::NoVersion => {
                ApiError::new(ErrorCode::NotFound, error.to_string())
            }
            error @ consume::ReadError::NotReady => {
                ApiError::new(ErrorCode::Conflict, error.to_string())
            }
        })?;
    install_instance::install_instance(
        &state.content,
        Arc::clone(&state.access.owed),
        id,
        busy,
        backend,
        version,
        InstanceInstall {
            instance_id: instance.id,
            instance_name: instance.name,
            shared_instance_id,
            properties,
            restart,
            force,
        },
    )
    .await?;
    Ok(())
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
    let server = server_rows::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let (hold, busy) = begin(&state, &id)?;
    if content_rows::get_pack(&state.db, &id).await?.is_some() {
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
                install_loader(
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
    content_rows::unlink(&state.db, &id).await?;
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
    get_linked_modpack_info(&state.content, &id, true)
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
    let pack =
        content_rows::get_pack(&state.db, &id)
            .await?
            .ok_or_else(|| {
                ApiError::new(
                    ErrorCode::NotFound,
                    "This server follows no pack.",
                )
            })?;
    let Some((project_id, installed_id)) = modrinth_pack(&pack.link) else {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "This pack came from a file; upload the new file instead.",
        ));
    };
    let no_update = || {
        ApiError::new(
            ErrorCode::Conflict,
            "The pack has no newer version on this server's update channel.",
        )
    };
    let installed = state
        .content
        .modrinth
        .version(installed_id)
        .await
        .map_err(content::Error::from)?;
    let newest =
        check_modpack_update(&state.content, &id, project_id, &installed)
            .await?
            .ok_or_else(no_update)?;
    let (hold, busy) = begin(&state, &id)?;
    spawn_pack(
        &state,
        hold,
        busy,
        PackSource::Modrinth {
            project_id: project_id.to_owned(),
            version_id: newest,
        },
        None,
        force,
    );
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::ACCEPTED)
}

/// Stream one multipart field into `path`, as much as fits on the disk (`archive::room`),
/// returning its SHA-1.
pub async fn stage_field(
    field: &mut Field<'_>,
    path: &std::path::Path,
) -> Result<String, ApiError> {
    let parent = path.parent().unwrap_or(path);
    tokio::fs::create_dir_all(parent).await?;
    let room = archive::room(parent)?;
    let result = async {
        let mut file = tokio::fs::File::create(path).await?;
        let mut hasher = sha1::Sha1::default();
        let mut written = 0u64;
        while let Some(chunk) = field.chunk().await.map_err(|error| {
            ApiError::new(ErrorCode::InvalidRequest, error.body_text())
        })? {
            written += chunk.len() as u64;
            if written > room {
                return Err(content::Error::from(NoRoom {
                    what: "the upload".into(),
                    needed: None,
                    room,
                })
                .into());
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
                    let sha1 = stage_field(&mut field, &path).await?;
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
        content_install::read_pack(
            &state.content,
            &stored,
            &Default::default(),
            false,
        )
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
        let mut n = 0;
        while let Some(mut field) = form.next_field().await.map_err(bad_form)? {
            let Some(filename) = field.file_name().map(str::to_owned) else {
                continue;
            };
            n += 1;
            let path = staging.join(n.to_string());
            stage_field(&mut field, &path).await?;
            add_uploaded_file(&state.content, &id, &filename, &path).await?;
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
    let files = list_edited_files(&state.content, &id).await?;
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
    let path = folder::clean(&decision.path).map_err(content::Error::from)?;
    decide_edited_file(
        &state.content,
        &id,
        &path,
        decision.choice == Choice::Replace,
    )
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// Query of GET `.../content/embedded-icon`. Mirrors `KyrosContentV1Module.getEmbeddedAddonIcon`.
#[derive(Debug, Clone, Deserialize)]
pub struct EmbeddedIconQuery {
    pub parent_directory: String,
    pub filename: String,
}

/// `GET /servers/{id}/v1/worlds/{wid}/content/embedded-icon`: the logo inside a mod or plugin
/// jar, for files Modrinth has no icon for. An addon's `manifest.icon_embedded` says it has one.
pub async fn embedded_icon(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<EmbeddedIconQuery>,
) -> Result<([(axum::http::HeaderName, &'static str); 1], Vec<u8>), ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    world(&id, &wid)?;
    let kind = match query.parent_directory.as_str() {
        "mods" => ContentType::Mod,
        "plugins" => ContentType::Plugin,
        _ => {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "parent_directory is mods or plugins.",
            ));
        }
    };
    let server = server_rows::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let loader = server.loader.as_deref().and_then(ModLoader::parse);
    let content = Arc::clone(&state.content);
    let root = state.servers.paths().server(&id);
    let icon = tokio::task::spawn_blocking(move || {
        let folder = folder::Folder::open(&root).ok()?;
        let listed = content_files::find(&folder, loader, &query.filename)
            .filter(|listed| listed.kind == kind)?;
        let key = format!("{id}/{}", listed.path);
        let path = content
            .manifests
            .get(
                &key,
                open_jar(&folder, &listed)?,
                loader.and_then(ModLoader::jar_platform),
            )?
            .icon?;
        let bytes = bedrock::upstream::jar::read_icon(
            open_jar(&folder, &listed)?,
            &path,
        )?;
        Some((path, bytes))
    })
    .await
    .ok()
    .flatten();
    let (path, bytes) = icon.ok_or_else(|| {
        ApiError::new(ErrorCode::NotFound, "That file has no embedded icon.")
    })?;
    let mime = match path
        .rsplit('.')
        .next()
        .map(str::to_ascii_lowercase)
        .as_deref()
    {
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        _ => "image/png",
    };
    Ok(([(CONTENT_TYPE, mime)], bytes))
}
