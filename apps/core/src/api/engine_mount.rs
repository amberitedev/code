//! The engine's content, share, install job list, file, upload, icon and log routes on a Core,
//! behind `CoreAccess`: every engine route declared `shared`, and no `client_only` one (the tests
//! check both). None carries a host path, reaches the engine's settings, store, caches, Java or
//! processes, or edits the instance, which writes launch hooks, Java and environment variables
//! (`api::servers` changes a server's name and update channel itself). A server's live console is
//! Core's own. A change is logged; a content change also holds the server, so it is refused while
//! the server runs or installs. Its [`Hook`] says what else it does.

use std::{io::Cursor, sync::Arc};

use axum::{
    extract::{FromRequestParts, RawPathParams, Request, State},
    handler::Handler,
    response::{IntoResponse, Response},
};
use contracts::{
    ApiError, ErrorCode, Json,
    backend::cores::ForgottenServer,
    core::{Action, ExportModpack},
    server::{Mount, Req},
};
use engine::{
    Engine,
    access::InstanceAccess,
    data::ResolveContentPlan,
    endpoints::{content::*, files::*, install::*, logs::*, sharing::*},
    routes::{self as engine_routes, api_error},
};
use futures::future::BoxFuture;
use uuid::Uuid;

use super::{CoreState, Shared, backups::zip_response, internal};
use crate::{
    access::{Caller, activity, owed::OwedCall},
    servers::{Hold, ServerError, server_rows},
};

#[derive(Clone, Copy)]
struct Hook {
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
    hold: true,
    backup: false,
    forget_players: false,
    job: false,
    icon: false,
};
const SHARE: Hook = Hook {
    hold: false,
    ..EDIT
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

/// Routes that answer within the request timeout. Each of these takes `Router::new()`; the tests
/// list what they mount.
pub fn routes<M: Mount<Shared>>(mount: M) -> M {
    use engine_routes::*;
    mount
        .endpoint::<ListContentItems, _, _>(list_content_items)
        .endpoint::<ListLinkedModpackContent, _, _>(list_linked_modpack_content)
        .endpoint::<RefreshContentUpdates, _, _>(refresh_content_updates)
        .endpoint::<GetSharedInstanceHistory, _, _>(get_shared_instance_history)
        .endpoint::<GetSharedInstancePublishPreview, _, _>(
            get_shared_instance_publish_preview,
        )
        .endpoint::<ToggleProject, _, _>(change(EDIT, toggle_project))
        .endpoint::<RemoveProject, _, _>(change(EDIT, remove_project))
        .endpoint::<SetSide, _, _>(change(EDIT, set_side))
        .endpoint::<SetSidesUnlocked, _, _>(change(EDIT, set_sides_unlocked))
        .endpoint::<UnlinkModpack, _, _>(change(EDIT, unlink_modpack))
        .endpoint::<DeleteSharedInstanceVersion, _, _>(change(
            SHARE,
            delete_shared_instance_version,
        ))
        .endpoint::<UnpublishSharedInstance, _, _>(change(
            SHARE,
            unpublish_shared_instance,
        ))
        .endpoint::<UnlinkSharedInstance, _, _>(change(
            Hook {
                forget_players: true,
                ..EDIT
            },
            unlink_shared_instance,
        ))
        .endpoint::<ListInstanceFiles, _, _>(list_instance_files)
        .endpoint::<StatInstanceFile, _, _>(stat_instance_file)
        .endpoint::<CreateInstanceDirectory, _, _>(change(
            FILE,
            create_instance_directory,
        ))
        .endpoint::<MoveInstanceFile, _, _>(change(FILE, move_instance_file))
        .endpoint::<DeleteInstanceFile, _, _>(change(
            FILE,
            delete_instance_file,
        ))
        .endpoint::<OpenInstanceUpload, _, _>(open_instance_upload)
        .endpoint::<GetInstanceUpload, _, _>(get_instance_upload)
        .endpoint::<CancelInstanceUpload, _, _>(cancel_instance_upload)
        .endpoint::<GetInstanceIcon, _, _>(get_instance_icon)
        .endpoint::<SetInstanceIcon, _, _>(change(ICON, set_instance_icon))
        .endpoint::<SetEditedInstanceIcon, _, _>(change(
            ICON,
            set_edited_instance_icon,
        ))
        .endpoint::<RemoveInstanceIcon, _, _>(change(
            ICON,
            remove_instance_icon,
        ))
        .endpoint::<DeleteInstanceLog, _, _>(change(FILE, delete_instance_log))
        .endpoint::<ListInstallJobs, _, _>(list_install_jobs)
}

/// The `untimed` routes: they download, or back the world up, first, or go through whole files and
/// folders, which can take longer than the request timeout, and reads at the same paths.
pub fn long_routes<M: Mount<Shared>>(mount: M) -> M {
    use engine_routes::*;
    mount
        .endpoint::<GetLinkedModpack, _, _>(get_linked_modpack)
        .endpoint::<AddProjectFromVersion, _, _>(change(
            EDIT,
            add_project_from_version,
        ))
        .endpoint::<InstallProjectWithDependencies, _, _>(change(
            EDIT,
            install_with_dependencies,
        ))
        .endpoint::<UpdateProject, _, _>(change(EDIT, update_project))
        .endpoint::<SwitchProjectVersion, _, _>(change(
            EDIT,
            switch_project_version,
        ))
        .endpoint::<RepairContent, _, _>(change(EDIT, repair_content))
        .endpoint::<UpdateInstanceContent, _, _>(change(
            JOB,
            update_instance_content,
        ))
        .endpoint::<RepairModpack, _, _>(change(JOB, repair_modpack))
        .endpoint::<InstallModpack, _, _>(change(BACKUP_JOB, install_modpack))
        .endpoint::<UpdateModpackVersion, _, _>(change(
            BACKUP_JOB,
            update_modpack_version,
        ))
        .endpoint::<RestoreSharedInstanceVersion, _, _>(change(
            BACKUP_JOB,
            restore_shared_instance_version,
        ))
        .endpoint::<ReadInstanceFile, _, _>(read_instance_file)
        .endpoint::<ExtractInstanceZip, _, _>(extract_zip)
        .endpoint::<ZipInstanceFiles, _, _>(change(FILE, zip_instance_files))
        .endpoint::<FinalizeInstanceUpload, _, _>(change(
            FILE,
            finalize_instance_upload,
        ))
        .endpoint::<ListInstanceLogs, _, _>(list_instance_logs)
        .endpoint::<GetInstanceLogOutput, _, _>(get_instance_log_output)
}

/// Routes whose body is a file of any size.
pub fn upload_routes<M: Mount<Shared>>(mount: M) -> M {
    use engine_routes::*;
    mount
        .endpoint::<WriteInstanceFile, _, _>(change(FILE, write_instance_file))
        .endpoint::<AddInstanceUploadFile, _, _>(add_instance_upload_file)
}

/// A dry run changes nothing, so only a real one is logged.
async fn extract_zip(
    State(state): State<Arc<CoreState>>,
    engine: State<Arc<Engine>>,
    instance: InstanceAccess,
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

/// [`ExportModpack`](contracts::core::ExportModpack): the server's content as an `.mrpack`, written
/// to its staging folder, streamed, then removed. A server exports what upstream's export selects
/// by default, with its plugins and its world's datapacks.
pub(super) async fn export_mrpack(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<ExportModpack>,
) -> Result<Response, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id).await?;
    let server = server_rows::get(&state.db, &state.engine, &id)
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
    let included = engine::instance::DEFAULT_SELECTED_EXPORT_PATH_PREFIXES
        .iter()
        .chain(&["plugins"])
        .map(|path| (*path).to_owned())
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

/// The engine's route answers before the content is written. Here the answer waits for it, so the
/// server stays held until it is and a failed download reaches the caller.
async fn install_with_dependencies(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess,
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
    state.access.require(&caller, &id).await?;
    if hook.hold && state.servers.engine_installing(&id).await? {
        return Err(ServerError::Installing.into());
    }
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
        // A panel that saw the server held reads it again once it is let go.
        drop(hold);
        let _ = engine::emit_instance(
            &state.engine.events,
            &id,
            engine::InstancePayloadType::Edited,
        )
        .await;
        return Ok(response);
    }
    let action = match hook.icon {
        true => Action::SettingsChanged,
        false => Action::ContentChanged,
    };
    activity::log(&state.db, &caller, Some(&id), action).await;
    if let Some(share_id) = share {
        forget_players(&state, &id, share_id).await?;
        super::follow::forget(&state, &id).await.map_err(internal)?;
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
pub(super) async fn forget_players(
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
            Ok(jobs) => {
                for job in jobs {
                    // Returns once the job ended, however it ended.
                    let _ = engine::install::runner::wait_for_job(
                        job.id,
                        &state.engine,
                    )
                    .await;
                }
            }
            Err(error) => {
                tracing::warn!(server = %id, %error, "could not follow the install");
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use axum::handler::Handler;
    use contracts::{Endpoint, Method, Reach, server::Mount};

    use super::Shared;

    /// What a mount function mounts.
    #[derive(Default)]
    struct Mounted(Vec<(Method, &'static str, bool)>);

    impl Mount<Shared> for Mounted {
        fn endpoint<E: Endpoint, H: Handler<T, Shared>, T: 'static>(
            mut self,
            _: H,
        ) -> Self {
            self.0.push((E::METHOD, E::PATH, E::UNTIMED));
            self
        }
    }

    #[test]
    fn a_core_mounts_every_shared_engine_route_and_no_other() {
        let engine_routes =
            || engine::endpoints::ROUTES.iter().copied().flatten();
        let mounted: BTreeSet<_> = [
            super::routes::<Mounted>,
            super::long_routes::<Mounted>,
            super::upload_routes::<Mounted>,
        ]
        .into_iter()
        .flat_map(|mount| mount(Mounted::default()).0)
        .map(|(method, path, _)| {
            engine_routes()
                .find(|route| route.method == method && route.path == path)
                .map_or(path, |route| route.name)
        })
        .collect();
        let shared: BTreeSet<_> = engine_routes()
            .filter(|route| route.reach == Some(Reach::Shared))
            .map(|route| route.name)
            .collect();
        assert_eq!(mounted, shared);
    }

    /// The TypeScript client times every call but the `untimed` ones out, so those are exactly the
    /// routes a Core mounts outside its request timeout.
    #[test]
    fn the_routes_outside_the_request_timeout_are_the_untimed_ones() {
        let mounted =
            |mount: fn(Mounted) -> Mounted| mount(Mounted::default()).0;
        let timed = mounted(crate::api::timed_routes::<Mounted>);
        let outside = mounted(crate::api::untimed_routes::<Mounted>);
        assert!(timed.iter().all(|&(.., untimed)| !untimed));
        assert!(outside.iter().all(|&(.., untimed)| untimed));
        let shared = engine::endpoints::ROUTES
            .iter()
            .copied()
            .flatten()
            .filter(|route| route.reach == Some(Reach::Shared));
        for route in contracts::core::ROUTES.iter().chain(shared) {
            if route.untimed {
                assert!(
                    outside.iter().any(|&(method, path, _)| {
                        method == route.method && path == route.path
                    }),
                    "{} is untimed but mounted inside the timeout",
                    route.name
                );
            }
        }
    }
}
