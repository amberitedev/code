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
        DeleteServer, GetServer, GetSetup, InstallLoader, JoinAddress,
        LinkedInstance, ListServers, PowerAction, ServerId, ServerNet,
        ServerPower, ServerSetup, UpdateChannel,
    },
    server::Req,
};
use modrinth_content_management::VersionType;

use super::{CoreState, internal};
use crate::{
    access::{Caller, Dangerous, Permission, activity, require_owner},
    installs::ModLoader,
    servers::{Phase, ServerError, launch},
    state::servers::{
        adapters::sqlite::{content_rows, server_rows},
        commands::{
            drop_own_share, install_loader, list_content::linked_pack_state,
            needs_update::needs_update,
        },
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

fn update_channel(channel: VersionType) -> UpdateChannel {
    match channel {
        VersionType::Release => UpdateChannel::Release,
        VersionType::Beta => UpdateChannel::Beta,
        VersionType::Alpha => UpdateChannel::Alpha,
    }
}

fn version_type(channel: UpdateChannel) -> VersionType {
    match channel {
        UpdateChannel::Release => VersionType::Release,
        UpdateChannel::Beta => VersionType::Beta,
        UpdateChannel::Alpha => VersionType::Alpha,
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
    Ok(CoreServer {
        net: net(state, &row),
        linked_instance: linked_instance(state, &row.id).await?,
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
        update_channel: update_channel(row.update_channel),
        id: row.id,
        name: row.name,
        owner_id,
        loader: row.loader,
        loader_version: row.loader_version,
        game_version: row.game_version,
        power,
        created_at,
    })
}

/// The App instance a server follows, when its pack is a version of that instance's share.
async fn linked_instance(
    state: &CoreState,
    id: &str,
) -> Result<Option<LinkedInstance>, ApiError> {
    let Some(pack) = content_rows::get_pack(&state.db, id)
        .await
        .map_err(internal)?
    else {
        return Ok(None);
    };
    let Some(linked) = pack.share.clone() else {
        return Ok(None);
    };
    Ok(Some(LinkedInstance {
        id: linked.instance_id,
        name: linked.instance_name,
        state: linked_pack_state(&state.content, id, &pack).await?,
        error: pack.error,
        shared_instance_id: Some(linked.shared_instance_id),
        version: Some(linked.version),
    }))
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
    let name = body.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "A server name has 1 to 64 characters.",
        ));
    }
    let row = state.servers.create(name).await?;
    activity::log(&state.db, &caller, Some(&row.id), Action::ServerCreated)
        .await;
    Ok(Json(ServerId { id: row.id }))
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
    if let Some(channel) = body.update_channel {
        server_rows::set_update_channel(&state.db, &id, version_type(channel))
            .await
            .map_err(internal)?;
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

/// Install a loader on a stopped server, as a bare install through the content routes does, but
/// answering once it is done.
pub async fn install(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Req { params, body, .. }: Req<InstallLoader>,
) -> Result<Empty, ApiError> {
    let id = params.id;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let Some(loader) = ModLoader::parse(&body.loader) else {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "Core installs Vanilla, Fabric, Quilt, Forge, NeoForge, and Paper.",
        ));
    };
    let busy = state.content.busy(&id)?;
    let hold = state.servers.hold(&id)?;
    content_rows::unlink(&state.db, &id)
        .await
        .map_err(internal)?;
    state.access.owed.notify_one();
    let progress = state.content.progress(&id);
    let result = install_loader::install_loader(
        &state.content,
        &hold,
        &progress,
        loader,
        &body.game_version,
        body.loader_version.as_deref(),
        false,
    )
    .await;
    progress.finish(
        result
            .as_ref()
            .err()
            .map(|error| format!("{error:#}"))
            .as_deref(),
    );
    result?;
    drop((hold, busy));
    activity::log(&state.db, &caller, Some(&id), Action::ContentChanged).await;
    Ok(Empty)
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
    state.servers.delete_folders(&id)?;
    Ok(Empty)
}

/// Delete a held server's row, and with it every role on it and its own share and that share's
/// history, owing the backend each removal and dropping its players. Returns who had a role.
async fn remove_server(
    state: &CoreState,
    caller: &Caller,
    id: &str,
) -> Result<Vec<String>, ApiError> {
    let mut tx = state.db.begin().await.map_err(internal)?;
    let share_id = drop_own_share::owe_share_deletion(&mut tx, id)
        .await
        .map_err(internal)?;
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
    if let Some(share_id) = share_id {
        drop_own_share::release_history(&state.content, &share_id).await;
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
    let paths = state.servers.paths();
    let installed = row.loader.is_some() && row.game_version.is_some();
    let (java_version, invocation) = if installed {
        (
            launch::java_major(paths, &state.content.http, &row)
                .await
                .ok(),
            launch::args(paths, &row)
                .ok()
                .map(|args| launch::invocation(&args)),
        )
    } else {
        (None, None)
    };
    // A server that follows an instance reads that instance's share; one that follows none has
    // its own once it is shared.
    let linked = content_rows::get_pack(&state.db, &id)
        .await
        .map_err(internal)?
        .and_then(|pack| pack.share);
    let (shared_instance_id, shared_instance_needs_update) = match linked {
        Some(linked) => (Some(linked.shared_instance_id), false),
        None => match server_rows::share_id(&state.db, &id)
            .await
            .map_err(internal)?
        {
            Some(share_id) => {
                (Some(share_id), needs_update(&state.content, &id).await)
            }
            None => (None, false),
        },
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
