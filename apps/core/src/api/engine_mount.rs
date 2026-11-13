//! The engine's content, share, file, upload and icon routes on a Core, behind `CoreAccess`. Only
//! the routes listed here are mounted: none whose body or answer carries a host path, none of the
//! engine-wide routes for settings, the store, caches, Java or processes, and not the engine's
//! edit-instance route, which writes launch hooks, Java and environment variables (`api::servers`
//! changes a server's name and update channel itself). A change is logged; a content change also
//! holds the server, so it is refused while the server runs or installs. Its [`Hook`] says what
//! else it does.

use std::{io::Cursor, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::{FromRequestParts, Path, RawPathParams, Request, State},
    handler::Handler,
    response::{IntoResponse, Response},
};
use contracts::{
    ApiError, ErrorCode, Json,
    backend::cores::ForgottenServer,
    core::Action,
    server::{Req, route},
};
use engine::{
    Engine,
    access::{Edit, InstanceAccess},
    data::ResolveContentPlan,
    endpoints::{content::*, files::*, install::*, sharing::*},
    instance::SharedInstancePublishPreview,
    routes::{self as engine_routes, api_error},
};
use futures::future::BoxFuture;
use uuid::Uuid;

use super::{CoreState, Shared, backups::zip_response, internal, world};
use crate::{
    access::{Caller, Permission, activity, owed::OwedCall},
    servers::{Hold, ServerError, server_rows},
};

#[derive(Clone, Copy)]
struct Hook {
    level: Permission,
    /// Refused while the server runs or installs (`Servers::hold`).
    hold: bool,
    /// Back the world up before the change.
    backup: bool,
    /// The share's players lose it: the server stops passing it on.
    forget_players: bool,
    /// The change is an engine install job, which installs the server software too. The server
    /// stays held until the job ends.
    job: bool,
    /// Logged as a settings change, and Minecraft's `server-icon.png` follows the icon.
    icon: bool,
}

const EDIT: Hook = Hook {
    level: Permission::Edit,
    hold: true,
    backup: false,
    forget_players: false,
    job: false,
    icon: false,
};
const OWNER: Hook = Hook {
    level: Permission::Owner,
    ..EDIT
};
const SHARE: Hook = Hook {
    hold: false,
    ..OWNER
};
const JOB: Hook = Hook { job: true, ..EDIT };
const BACKUP_JOB: Hook = Hook {
    backup: true,
    ..JOB
};
/// A server's files change while it runs, as they would by hand.
const FILE: Hook = Hook {
    hold: false,
    ..EDIT
};
const ICON: Hook = Hook { icon: true, ..FILE };

/// Routes that answer within the request timeout.
pub fn routes() -> Router<Shared> {
    use engine_routes::*;
    [
        route::<ListContentItems, _, _, _>(list_content_items),
        route::<ListLinkedModpackContent, _, _, _>(list_linked_modpack_content),
        route::<RefreshContentUpdates, _, _, _>(refresh_content_updates),
        route::<GetSharedInstanceHistory, _, _, _>(get_shared_instance_history),
        route::<GetSharedInstancePublishPreview, _, _, _>(publish_preview),
        route::<ToggleProject, _, _, _>(change(EDIT, toggle_project)),
        route::<RemoveProject, _, _, _>(change(EDIT, remove_project)),
        route::<SetSide, _, _, _>(change(EDIT, set_side)),
        route::<SetSidesUnlocked, _, _, _>(change(EDIT, set_sides_unlocked)),
        route::<CompleteSideReview, _, _, _>(change(
            EDIT,
            complete_side_review,
        )),
        route::<UnlinkModpack, _, _, _>(change(EDIT, unlink_modpack)),
        route::<DeleteSharedInstanceVersion, _, _, _>(change(
            SHARE,
            delete_shared_instance_version,
        )),
        route::<UnpublishSharedInstance, _, _, _>(change(
            SHARE,
            unpublish_shared_instance,
        )),
        route::<UnlinkSharedInstance, _, _, _>(change(
            Hook {
                forget_players: true,
                ..OWNER
            },
            unlink_shared_instance,
        )),
        route::<ListInstanceFiles, _, _, _>(list_instance_files),
        route::<StatInstanceFile, _, _, _>(stat_instance_file),
        route::<CreateInstanceDirectory, _, _, _>(change(
            FILE,
            create_instance_directory,
        )),
        route::<MoveInstanceFile, _, _, _>(change(FILE, move_instance_file)),
        route::<DeleteInstanceFile, _, _, _>(change(
            FILE,
            delete_instance_file,
        )),
        route::<OpenInstanceUpload, _, _, _>(open_instance_upload),
        route::<GetInstanceUpload, _, _, _>(get_instance_upload),
        route::<CancelInstanceUpload, _, _, _>(cancel_instance_upload),
        route::<GetInstanceIcon, _, _, _>(get_instance_icon),
        route::<SetInstanceIcon, _, _, _>(change(ICON, set_instance_icon)),
        route::<SetEditedInstanceIcon, _, _, _>(change(
            ICON,
            set_edited_instance_icon,
        )),
        route::<RemoveInstanceIcon, _, _, _>(change(
            ICON,
            remove_instance_icon,
        )),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

/// Routes that download, or back the world up, first, or go through whole files and folders, which
/// can take longer than the request timeout, and reads at the same paths.
pub fn long_routes() -> Router<Shared> {
    use engine_routes::*;
    [
        route::<GetLinkedModpack, _, _, _>(get_linked_modpack),
        route::<EditedFiles, _, _, _>(edited_files),
        route::<AddProjectFromVersion, _, _, _>(change(
            EDIT,
            add_project_from_version,
        )),
        route::<InstallProjectWithDependencies, _, _, _>(change(
            EDIT,
            install_with_dependencies,
        )),
        route::<UpdateProject, _, _, _>(change(EDIT, update_project)),
        route::<SwitchProjectVersion, _, _, _>(change(
            EDIT,
            switch_project_version,
        )),
        route::<DecideEdited, _, _, _>(change(EDIT, decide_edited)),
        route::<PlaceClientOnly, _, _, _>(change(EDIT, place_client_only)),
        route::<RepairContent, _, _, _>(change(EDIT, repair_content)),
        route::<UpdateInstanceContent, _, _, _>(change(
            JOB,
            update_instance_content,
        )),
        route::<RepairModpack, _, _, _>(change(JOB, repair_modpack)),
        route::<InstallModpack, _, _, _>(change(BACKUP_JOB, install_modpack)),
        route::<UpdateModpackVersion, _, _, _>(change(
            BACKUP_JOB,
            update_modpack_version,
        )),
        route::<RestoreSharedInstanceVersion, _, _, _>(change(
            Hook {
                level: Permission::Owner,
                ..BACKUP_JOB
            },
            restore_shared_instance_version,
        )),
        route::<ReadInstanceFile, _, _, _>(read_instance_file),
        route::<ExtractInstanceZip, _, _, _>(extract_zip),
        route::<ZipInstanceFiles, _, _, _>(change(FILE, zip_instance_files)),
        route::<FinalizeInstanceUpload, _, _, _>(change(
            FILE,
            finalize_instance_upload,
        )),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
    .route(
        "/servers/{id}/v1/worlds/{wid}/content/export-mrpack",
        axum::routing::get(export_mrpack),
    )
}

/// Routes whose body is a file of any size.
pub fn upload_routes() -> Router<Shared> {
    use engine_routes::*;
    [
        route::<WriteInstanceFile, _, _, _>(change(FILE, write_instance_file)),
        route::<AddInstanceUploadFile, _, _, _>(add_instance_upload_file),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

/// A dry run changes nothing, so only a real one is logged.
async fn extract_zip(
    State(state): State<Arc<CoreState>>,
    engine: State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<ExtractInstanceZip>,
) -> Result<Json<Option<ExtractDryRunResult>>, ApiError> {
    let (caller, id) = (instance.caller.clone(), instance.id.clone());
    let dry_run = request.body.dry_run;
    let extracted =
        engine_routes::extract_instance_zip(engine, instance, request).await?;
    if !dry_run {
        activity::log(&state.db, &caller, Some(&id), Action::ContentChanged)
            .await;
    }
    Ok(extracted)
}

/// `GET /servers/{id}/v1/worlds/{wid}/content/export-mrpack`: the server's content as an
/// `.mrpack`, written to its staging folder, streamed, then removed. The App saves it natively, so
/// it asks for a link first (`access::links`). A server exports what upstream's export selects by
/// default: its content folders and `config/`.
async fn export_mrpack(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    world(&id, &wid)?;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let server = server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let staging = state.servers.paths().staging(&id);
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(internal)?;
    let path = staging.join(format!("export-{}.mrpack", Uuid::new_v4()));
    let level =
        engine::sandbox::Folder::open(&state.servers.paths().server(&id))
            .map_err(internal)?
            .level_name();
    let included = ["mods", "plugins", "config"]
        .map(str::to_owned)
        .into_iter()
        .chain([format!("{level}/datapacks")])
        .collect();
    let exported = engine::instance::export_mrpack(
        &state.engine,
        &id,
        path.clone(),
        included,
        Vec::new(),
        None,
        None,
        None,
    )
    .await;
    if let Err(error) = exported {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(api_error(error));
    }
    let file = std::fs::File::open(&path).map_err(internal)?;
    Ok(zip_response(file, &server.name, "mrpack", Some(path)))
}

/// What a push would change, for editors: the engine's route is for the share's owner, and on a
/// Core the owner is the Core.
async fn publish_preview(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<GetSharedInstancePublishPreview>,
) -> Result<Json<Option<SharedInstancePublishPreview>>, ApiError> {
    engine::instance::get_shared_instance_publish_preview(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

/// The engine's route answers before the content is written. Here the answer waits for it, so the
/// server stays held until it is and a failed download reaches the caller.
async fn install_with_dependencies(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<InstallProjectWithDependencies>,
) -> Result<Json<ResolveContentPlan>, ApiError> {
    let plan = engine::instance::resolve_project_with_dependencies(
        &engine,
        &instance.id,
        request.body,
    )
    .await
    .map_err(api_error)?;
    engine::instance::install_content_plan(&engine, &instance.id, &plan)
        .await
        .map_err(api_error)?;
    Ok(Json(plan))
}

/// `handler` behind `hook`.
fn change<H, T>(
    hook: Hook,
    handler: H,
) -> impl FnOnce(State<Shared>, Request) -> BoxFuture<'static, Response>
+ Clone
+ Send
+ Sync
+ 'static
where
    H: Handler<T, Shared>,
    T: 'static,
{
    move |State(shared), request| {
        Box::pin(async move {
            hooked(hook, handler, shared, request)
                .await
                .unwrap_or_else(IntoResponse::into_response)
        })
    }
}

async fn hooked<H, T>(
    hook: Hook,
    handler: H,
    shared: Shared,
    request: Request,
) -> Result<Response, ApiError>
where
    H: Handler<T, Shared>,
    T: 'static,
{
    let (mut parts, body) = request.into_parts();
    let caller = Caller::from_request_parts(&mut parts, &shared).await?;
    let id = RawPathParams::from_request_parts(&mut parts, &shared)
        .await
        .map_err(|rejection| {
            ApiError::new(ErrorCode::InvalidInput, rejection.body_text())
        })?
        .iter()
        .find_map(|(name, value)| (name == "id").then(|| value.to_owned()))
        .expect("an instance route has an {id} segment");
    let state = Arc::clone(&shared.0);
    state.access.require(&caller, &id, hook.level).await?;
    let hold = hook.hold.then(|| state.servers.hold(&id)).transpose()?;
    if hook.backup {
        state.backups.pre_update(&id).await?;
    }
    let share = match hook.forget_players {
        true => engine::instance::get(&state.engine, &id)
            .await
            .map_err(api_error)?
            .and_then(|instance| instance.shared_instance)
            .map(|share| share.id),
        false => None,
    };
    let response = handler.call(Request::from_parts(parts, body), shared).await;
    if !response.status().is_success() {
        return Ok(response);
    }
    let action = match hook.icon {
        true => Action::SettingsChanged,
        false => Action::ContentChanged,
    };
    activity::log(&state.db, &caller, Some(&id), action).await;
    if let Some(share_id) = share {
        forget_players(&state, &id, share_id).await?;
    }
    if hook.icon
        && let Err(error) = game_icon(&state, &id).await
    {
        tracing::warn!(server = %id, error = %error.description, "could not write server-icon.png");
    }
    if let Some(hold) = hold.filter(|_| hook.job) {
        tokio::spawn(finish_job(state, hold));
    }
    Ok(response)
}

/// Write the server's icon as Minecraft's `server-icon.png`, the 64x64 PNG it shows in the
/// multiplayer list from its next start, or remove it with the icon.
async fn game_icon(state: &CoreState, id: &str) -> Result<(), ApiError> {
    const GAME_ICON: &str = "server-icon.png";
    let icon = engine::instance::read_icon(&state.engine, id)
        .await
        .map_err(api_error)?;
    let folder =
        engine::sandbox::Folder::open(&state.servers.paths().server(id))
            .map_err(internal)?;
    tokio::task::spawn_blocking(move || match icon {
        Some(icon) => {
            let mut png = Vec::new();
            image::load_from_memory(&icon)
                .map_err(internal)?
                .resize_to_fill(64, 64, image::imageops::FilterType::Lanczos3)
                .write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png)
                .map_err(internal)?;
            folder
                .write(GAME_ICON, &mut png.as_slice())
                .map(drop)
                .map_err(internal)
        }
        // Only a plain file: `remove` would also take a folder by that name, and its contents.
        None if folder.metadata(GAME_ICON).is_ok_and(|meta| meta.is_file()) => {
            folder.remove(GAME_ICON).map_err(internal)
        }
        None => Ok(()),
    })
    .await
    .map_err(internal)?
}

/// Owe the backend the removal of the players the server passed `share_id` on to.
async fn forget_players(
    state: &CoreState,
    id: &str,
    share_id: String,
) -> Result<(), ApiError> {
    let mut conn = state.db.acquire().await.map_err(internal)?;
    crate::access::sync::owe(
        &mut conn,
        &OwedCall::ForgetPlayers(ForgottenServer {
            server_id: id.to_owned(),
            share_id: Some(share_id),
        }),
    )
    .await
    .map_err(internal)?;
    state.access.owed.notify_one();
    Ok(())
}

/// Hold the server until the engine's install jobs on it end. Once the server is let go, its
/// instance reads as edited, so a panel fetches it again and sees the install done.
pub(super) async fn finish_job(state: Arc<CoreState>, hold: Hold) {
    let id = hold.id().to_owned();
    wait_for_jobs(&state, &id).await;
    drop(hold);
    let _ = engine::emit_instance(
        &state.engine.events,
        &id,
        engine::InstancePayloadType::Edited,
    )
    .await;
}

async fn wait_for_jobs(state: &CoreState, id: &str) {
    loop {
        match engine::install::store::list_active_for_instance(
            id,
            state.engine.pool(),
        )
        .await
        {
            Ok(jobs) if jobs.is_empty() => break,
            Ok(_) => tokio::time::sleep(Duration::from_secs(1)).await,
            Err(error) => {
                tracing::warn!(server = %id, %error, "could not follow the install");
                return;
            }
        }
    }
}
