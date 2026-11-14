//! Servers: listing, creating and deleting them, power, a loader install, the update channel and
//! the setup a server runs with.

use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

use axum::extract::State;
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    core::{
        Action, ChangePower, ChangeServer, CoreRole, CoreServer, CreateServer,
        DeleteServer, GetServer, GetSetup, InstallLoader, InstallModpackFile,
        JoinAddress, ListServers, PowerAction, ServerId, ServerNet,
        ServerPower, ServerSetup, ShareServer, SharedServer, UpdateChannel,
    },
    server::Req,
};
use engine::{
    routes::api_error,
    state::{ReleaseChannel, SharedInstanceRole},
};
use futures::TryStreamExt;
use tokio::io::AsyncWriteExt;

use super::{CoreState, engine_mount, internal};
use crate::{
    access::{Caller, Dangerous, Permission, activity, require_owner},
    servers::{
        Phase, ServerError, launch,
        platform::{self, Platform},
        server_rows,
    },
};

impl From<ServerError> for ApiError {
    fn from(error: ServerError) -> Self {
        let code = match &error {
            ServerError::NotFound(_) => ErrorCode::NotFound,
            ServerError::AlreadyRunning => ErrorCode::AlreadyRunning,
            ServerError::Installing => ErrorCode::Installing,
            ServerError::Unmanaged => ErrorCode::NotManaged,
            ServerError::NoPorts => ErrorCode::NoPorts,
            ServerError::Eula => ErrorCode::EulaNotAccepted,
            ServerError::NotRunning
            | ServerError::Starting
            | ServerError::BackingUp
            | ServerError::NotReady(_)
            | ServerError::Shim(_) => ErrorCode::Conflict,
            ServerError::StartFailed(_) | ServerError::Other(_) => {
                ErrorCode::InternalError
            }
        };
        Self::new(code, format!("{error:#}"))
    }
}

fn update_channel(channel: ReleaseChannel) -> UpdateChannel {
    match channel {
        ReleaseChannel::Release => UpdateChannel::Release,
        ReleaseChannel::Beta => UpdateChannel::Beta,
        ReleaseChannel::Alpha => UpdateChannel::Alpha,
    }
}

fn release_channel(channel: UpdateChannel) -> ReleaseChannel {
    match channel {
        UpdateChannel::Release => ReleaseChannel::Release,
        UpdateChannel::Beta => ReleaseChannel::Beta,
        UpdateChannel::Alpha => ReleaseChannel::Alpha,
    }
}

// Every route below checks the caller's role on the server, or that they own the Core.

/// Where players join a server from the internet: the router's grant when it gave a different
/// outside port than the game port, otherwise the Core's join IP and the game port.
fn join_address(
    join_ip: Option<IpAddr>,
    game_port: u16,
    outside: Option<SocketAddr>,
) -> (Option<IpAddr>, u16) {
    match outside {
        Some(outside) if outside.port() != game_port => {
            (Some(outside.ip()), outside.port())
        }
        _ => (join_ip, game_port),
    }
}

fn net(state: &CoreState, row: &server_rows::ServerRow) -> ServerNet {
    let game_port = u16::try_from(row.port).unwrap_or_default();
    // Known while the server runs and its port is forwarded, or open on a public address.
    let outside = state
        .network
        .report(&row.id)
        .and_then(|report| report.outside);
    let (ip, port) = join_address(state.network.join_ip(), game_port, outside);
    ServerNet {
        ip: ip.map(|ip| ip.to_string()),
        port,
        outside: outside.map(|address| JoinAddress {
            ip: address.ip().to_string(),
            port: address.port(),
        }),
        game_port,
    }
}

/// Where `id` is in its run, from its `phase`, or how its last run ended when it has none.
pub(super) async fn power_state(
    state: &CoreState,
    id: &str,
    phase: Option<Phase>,
) -> sqlx::Result<ServerPower> {
    Ok(match phase {
        Some(Phase::Starting) => ServerPower::Starting,
        Some(Phase::Running | Phase::Unmanaged(_)) => ServerPower::Running,
        Some(Phase::Stopping) => ServerPower::Stopping,
        Some(Phase::Installing) => ServerPower::Offline,
        None => match server_rows::last_outcome(&state.db, id).await? {
            Some(server_rows::Outcome::Crashed) => ServerPower::Crashed,
            Some(server_rows::Outcome::Killed) => ServerPower::Killed,
            Some(server_rows::Outcome::Stopped) | None => ServerPower::Offline,
        },
    })
}

/// A server row as a caller with `level` on it sees it.
async fn server(
    state: &CoreState,
    row: server_rows::ServerRow,
    level: Permission,
) -> Result<CoreServer, ApiError> {
    let phase = state.servers.phase(&row.id);
    let power = power_state(state, &row.id, phase).await.map_err(internal)?;
    let owner_id: Option<String> =
        sqlx::query_scalar("SELECT owner_user_id FROM core WHERE id = 1")
            .fetch_optional(&state.db)
            .await
            .map_err(internal)?
            .flatten();
    let created_at: i64 =
        sqlx::query_scalar("SELECT created_at FROM servers WHERE id = ?")
            .bind(&row.id)
            .fetch_one(&state.db)
            .await
            .map_err(internal)?;
    let eula_accepted = super::eula::accepted_at(&state.db)
        .await
        .map_err(internal)?
        .is_some();
    let instance = engine::instance::get(&state.engine, &row.id)
        .await
        .map_err(api_error)?
        .ok_or_else(|| ServerError::NotFound(row.id.clone()))?;
    let set = instance.applied_content_set;
    let installed = !set.game_version.is_empty();
    Ok(CoreServer {
        net: net(state, &row),
        linked_instance: None,
        reachability: state
            .network
            .report(&row.id)
            .map(contracts::core::ReachabilityReport::from),
        players: state
            .servers
            .insight(&row.id)
            .map(|insight| insight.players.into_iter().collect()),
        role: match level {
            Permission::Owner => None,
            Permission::Edit => Some(CoreRole::Editor),
            Permission::View => Some(CoreRole::Viewer),
        },
        installing: phase == Some(Phase::Installing),
        memory_mb: u32::try_from(row.memory_mb).unwrap_or_default(),
        update_channel: update_channel(instance.instance.update_channel),
        id: row.id,
        name: row.name,
        owner_id,
        loader: installed.then(|| set.loader.as_str().to_owned()),
        loader_version: set.loader_version.filter(|_| installed),
        game_version: installed.then_some(set.game_version),
        power,
        created_at,
        eula_accepted,
    })
}

pub async fn list(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<ListServers>,
) -> Result<Json<Vec<CoreServer>>, ApiError> {
    let mut servers = Vec::new();
    for row in server_rows::list(&state.db).await.map_err(internal)? {
        if let Some(level) = state
            .access
            .level(&caller, &row.id)
            .await
            .map_err(internal)?
        {
            servers.push(server(&state, row, level).await?);
        }
    }
    Ok(Json(servers))
}

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<GetServer>,
) -> Result<Json<CoreServer>, ApiError> {
    let id = params.id;
    let level = state.access.require(&caller, &id, Permission::View).await?;
    let row = server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or(ServerError::NotFound(id))?;
    Ok(Json(server(&state, row, level).await?))
}

pub async fn create(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { body, .. }: Req<CreateServer>,
) -> Result<Json<ServerId>, ApiError> {
    require_owner(&caller)?;
    let row = state.servers.create(server_name(&body.name)?).await?;
    activity::log(&state.db, &caller, Some(&row.id), Action::ServerCreated)
        .await;
    Ok(Json(ServerId { id: row.id }))
}

fn server_name(name: &str) -> Result<&str, ApiError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "A server name has 1 to 64 characters.",
        ));
    }
    Ok(name)
}

pub async fn change(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<ChangeServer>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    if let Some(name) = &body.name {
        server_rows::set_name(&state.db, &id, server_name(name)?)
            .await
            .map_err(internal)?;
        activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged)
            .await;
    }
    if let Some(channel) = body.update_channel {
        engine::instance::edit(
            &state.engine,
            &id,
            engine::state::EditInstance {
                update_channel: Some(release_channel(channel)),
                ..Default::default()
            },
        )
        .await
        .map_err(api_error)?;
        activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged)
            .await;
    }
    Ok(Empty)
}

pub async fn power(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<ChangePower>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let action = match body.action {
        PowerAction::Start => Action::ServerStarted,
        PowerAction::Stop => Action::ServerStopped,
        PowerAction::Kill => Action::ServerKilled,
        PowerAction::Restart => Action::ServerRestarted,
    };
    match body.action {
        PowerAction::Start => state.servers.start(&id).await?,
        PowerAction::Stop => state.servers.stop(&id).await?,
        PowerAction::Kill => state.servers.kill(&id).await?,
        PowerAction::Restart => {
            if state.servers.phase(&id).is_none() {
                return Err(ServerError::NotRunning.into());
            }
            let (state, id) = (Arc::clone(&state), id.clone());
            tokio::spawn(async move {
                if let Err(error) = state.servers.restart(&id).await {
                    tracing::warn!(server = %id, %error, "restart failed");
                }
            });
        }
    }
    activity::log(&state.db, &caller, Some(&id), action).await;
    Ok(Empty)
}

/// Install a loader on a stopped server through the engine's install job for it, answering once
/// the job is done. Its progress reaches the event socket as the job's events.
pub async fn install(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<InstallLoader>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let Some(loader) = platform::parse_loader(&body.loader) else {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "Core installs Vanilla, Fabric, Quilt, Forge, NeoForge, and Paper.",
        ));
    };
    if state.servers.engine_installing(&id).await? {
        return Err(ServerError::Installing.into());
    }
    let hold = state.servers.hold(&id)?;
    server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    if platform::platform(&state.engine, &id)
        .await
        .map_err(api_error)?
        .is_some()
    {
        state.backups.pre_update(&id).await?;
    }
    let platform = Platform {
        loader,
        game_version: body.game_version,
        loader_version: body.loader_version,
    };
    // Not cancelled with the request: the job goes on, so the hold and the restore must too.
    let engine = Arc::clone(&state.engine);
    tokio::spawn(async move {
        let installed = platform::install(&engine, hold.id(), &platform).await;
        drop(hold);
        installed
    })
    .await
    .map_err(internal)?
    .map_err(api_error)?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(Empty)
}

/// Install the `.mrpack` in the body over the stopped server, in the background. It is staged in
/// the server's run folder, which the server cannot write, with the disk's free space as its cap.
pub async fn install_modpack_file(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<InstallModpackFile>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let hold = state.servers.hold(&id)?;
    let dir = state.servers.paths().run(&id);
    tokio::fs::create_dir_all(&dir).await.map_err(internal)?;
    let pack = dir.join("upload.mrpack");
    let mut room = engine::archive::room(&dir).map_err(internal)?;
    let mut file = tokio::fs::File::create(&pack).await.map_err(internal)?;
    let mut chunks = body.into_data_stream();
    while let Some(chunk) = chunks.try_next().await.map_err(|error| {
        ApiError::new(ErrorCode::InvalidInput, error.to_string())
    })? {
        room = room.checked_sub(chunk.len() as u64).ok_or_else(|| {
            ApiError::new(
                ErrorCode::InvalidInput,
                "The pack does not fit on the Core's disk.",
            )
        })?;
        file.write_all(&chunk).await.map_err(internal)?;
    }
    file.flush().await.map_err(internal)?;
    drop(file);
    if platform::platform(&state.engine, &id)
        .await
        .map_err(api_error)?
        .is_some()
    {
        state.backups.pre_update(&id).await?;
    }
    engine::install::install_pack_to_existing_instance(
        id.clone(),
        engine::pack::install_from::CreatePackLocation::FromFile { path: pack },
        None,
        &state.engine,
    )
    .await
    .map_err(api_error)?;
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    tokio::spawn(engine_mount::finish_job(Arc::clone(&state), hold));
    Ok(Empty)
}

/// Push a version of the server's own share. The first push makes the share with a version of
/// its own; the files it picks go out in a second.
pub async fn share(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<ShareServer>,
) -> Result<Json<SharedServer>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let instance = engine::instance::get(&state.engine, &id)
        .await
        .map_err(api_error)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let shared = instance.shared_instance.is_some();
    if state.servers.phase(&id) == Some(Phase::Installing) {
        if shared {
            return Err(ServerError::Installing.into());
        }
        // The share is made now, so it can be passed on, and its first version once the install
        // let the server go, so it carries what was installed.
        let attachment = engine::instance::ensure_unpublished_shared_instance(
            &state.engine,
            &id,
        )
        .await
        .map_err(api_error)?;
        activity::log(&state.db, &caller, Some(&id), Action::ContentChanged)
            .await;
        tokio::spawn(publish_after_install(Arc::clone(&state), id));
        return Ok(Json(SharedServer {
            shared_instance_id: attachment.id,
            version: None,
            restarted: false,
        }));
    }
    if instance.applied_content_set.game_version.is_empty() {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "Install Minecraft on this server before sharing it.",
        ));
    }
    let mut attachment =
        engine::instance::ensure_shared_instance(&state.engine, &id)
            .await
            .map_err(api_error)?;
    if shared || !body.file_paths.is_empty() || !body.removed_paths.is_empty() {
        let core_id = crate::db::core_id(&state.db).await.map_err(internal)?;
        attachment = engine::instance::publish_shared_instance(
            &state.engine,
            &id,
            body.file_paths,
            body.removed_paths,
            Some(core_id),
            &|_, _| {},
        )
        .await
        .map_err(api_error)?;
    }
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    let restarted = body.restart && state.servers.phase(&id).is_some();
    if restarted {
        let (state, id) = (Arc::clone(&state), id.clone());
        tokio::spawn(async move {
            if let Err(error) = state.servers.restart(&id).await {
                tracing::warn!(server = %id, %error, "restart after a push failed");
            }
        });
    }
    Ok(Json(SharedServer {
        shared_instance_id: attachment.id,
        version: attachment.latest_version,
        restarted,
    }))
}

/// Publish the first version of a share made while the server installed, once it is let go.
async fn publish_after_install(state: Arc<CoreState>, id: String) {
    while state.servers.phase(&id) == Some(Phase::Installing) {
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    let published = async {
        let core_id = crate::db::core_id(&state.db).await.map_err(internal)?;
        engine::instance::publish_shared_instance(
            &state.engine,
            &id,
            Vec::new(),
            Vec::new(),
            Some(core_id),
            &|_, _| {},
        )
        .await
        .map_err(api_error)
    }
    .await;
    if let Err(error) = published {
        tracing::warn!(server = %id, error = %error.description, "could not publish the share's first version");
    }
}

/// Its activity history stays.
pub async fn delete(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<DeleteServer>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    if state.servers.hold_for_deletion(&id).is_err() {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "Stop the server before deleting it.",
        ));
    }
    let removed = remove_server(&state, &caller, &id).await;
    let members = match removed {
        Ok(members) => members,
        Err(error) => {
            state.servers.release(&id);
            return Err(error);
        }
    };
    state.access.owed.notify_one();
    for user in &members {
        state.access.changed(user);
    }
    if let Err(error) = state.servers.delete_folders(&id).await {
        tracing::warn!(server = %id, %error, "could not remove a deleted server's files");
        return Err(ApiError::new(
            ErrorCode::InternalError,
            "The server was deleted, but some of its files could not be removed yet. Core removes them when it next starts.",
        ));
    }
    Ok(Empty)
}

/// Delete a held server's row, and with it every role on it and its own share's history, owing
/// the backend each removal and the share's deletion and dropping its players. Returns who had a
/// role.
async fn remove_server(
    state: &CoreState,
    caller: &Caller,
    id: &str,
) -> Result<Vec<String>, ApiError> {
    let own_share = engine::instance::get(&state.engine, id)
        .await
        .map_err(api_error)?
        .and_then(|instance| instance.shared_instance)
        .filter(|share| share.role == SharedInstanceRole::Owner)
        .map(|share| share.id);
    let mut tx = state.db.begin().await.map_err(internal)?;
    if let Some(share_id) = &own_share {
        crate::access::sync::owe(
            &mut tx,
            &crate::access::owed::OwedCall::DeleteShare {
                share_id: share_id.clone(),
            },
        )
        .await
        .map_err(internal)?;
    }
    let members: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM access WHERE server_id = ?")
            .bind(id)
            .fetch_all(&mut *tx)
            .await
            .map_err(internal)?;
    sqlx::query("DELETE FROM servers WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    // Its players, if it had any, lose what it shared with them.
    crate::access::sync::owe(
        &mut tx,
        &crate::access::owed::OwedCall::ForgetPlayers(
            contracts::backend::cores::ForgottenServer {
                server_id: id.into(),
                share_id: None,
            },
        ),
    )
    .await
    .map_err(internal)?;
    for user in &members {
        super::access::owe_removal(&mut tx, user, id)
            .await
            .map_err(internal)?;
    }
    activity::record(
        &mut tx,
        caller.user_id(),
        Some(id),
        &Action::ServerDeleted,
    )
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    if let Some(share_id) = own_share
        && let Err(error) =
            engine::instance::forget_share_history(&share_id, &state.engine)
                .await
    {
        tracing::warn!(server = %id, %error, "could not forget the share's history");
    }
    Ok(members)
}

pub async fn setup(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, .. }: Req<GetSetup>,
) -> Result<Json<ServerSetup>, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::View).await?;
    let row = server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let (java_version, invocation) =
        match platform::platform(&state.engine, &id)
            .await
            .map_err(api_error)?
        {
            Some(platform) => (
                launch::java_major(&state.engine, &row, &platform.game_version)
                    .await
                    .ok(),
                engine::server::server_command(&state.engine, &id)
                    .await
                    .ok()
                    .map(|command| {
                        launch::invocation(&launch::args(
                            state.servers.paths(),
                            &row,
                            None,
                            command.args,
                        ))
                    }),
            ),
            None => (None, None),
        };
    let shared_instance_id = engine::instance::get(&state.engine, &id)
        .await
        .map_err(api_error)?
        .and_then(|instance| instance.shared_instance)
        .map(|share| share.id);
    // Best effort: what a push would change, as the share's latest version reads.
    let shared_instance_needs_update = match &shared_instance_id {
        Some(_) => engine::instance::get_shared_instance_publish_preview(
            &state.engine,
            &id,
        )
        .await
        .ok()
        .flatten()
        .is_some_and(|preview| !preview.diffs.is_empty()),
        None => false,
    };
    Ok(Json(ServerSetup {
        java_version,
        invocation,
        shared_instance_id,
        shared_instance_needs_update,
        cpus: std::thread::available_parallelism()
            .map_or(1, |cpus| cpus.get() as u32),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_address_uses_the_routers_port_only_when_it_differs() {
        let public: IpAddr = "203.0.113.5".parse().unwrap();
        let wan: IpAddr = "198.51.100.7".parse().unwrap();
        assert_eq!(
            join_address(Some(public), 25565, None),
            (Some(public), 25565)
        );
        assert_eq!(
            join_address(
                Some(public),
                25565,
                Some(SocketAddr::new(wan, 25565))
            ),
            (Some(public), 25565)
        );
        assert_eq!(
            join_address(
                Some(public),
                25565,
                Some(SocketAddr::new(wan, 41000))
            ),
            (Some(wan), 41000)
        );
    }
}
