//! Servers: `GET /modrinth/v0/servers`, `GET /modrinth/v0/servers/{id}` and its `power`, `name`,
//! `fs`, `ws`, and `reinstall` routes; `GET /v1/servers`, `GET /v1/servers/{id}`,
//! `GET /v1/servers/{id}/worlds/{wid}/select-download`, `DELETE /v1/servers/{id}/flows/intro`; and
//! Amberite's `POST /local/servers` and `DELETE /local/servers/{id}`. Mirrors `Archon.Servers` in
//! `packages/api-client/src/modules/archon/types.ts` (`archon/servers/v0.ts`, `archon/servers/v1.ts`,
//! and `createServer` in `archon/cores/v1.ts`).

#![allow(dead_code)] // Wired in step 4: worlds, files, and linked instances.

use std::{collections::BTreeMap, sync::Arc};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode, header::HOST},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::{
    AppState,
    activity::Action,
    content::LinkedPackState,
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Dangerous, Level, activity},
    content,
    installs::ModLoader,
    servers::{Phase, ServerError, launch, records},
};

/// Query of GET `/modrinth/v0/servers`. Mirrors Archon.Servers.v0.GetServersOptions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerListQuery {
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

/// Response of GET `/modrinth/v0/servers`. `users` is keyed by user id. Mirrors
/// Archon.Servers.v0.ServerGetResponse.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerGetResponse {
    pub servers: Vec<Server>,
    pub pagination: Pagination,
    pub users: BTreeMap<String, ServerOwner>,
}

/// Mirrors Archon.Servers.v0.Pagination.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Pagination {
    pub current_page: u32,
    pub page_size: u32,
    pub total_pages: u32,
    pub total_items: u32,
}

/// Mirrors Archon.Servers.v0.ServerOwner.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerOwner {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// Mirrors Archon.Servers.v0.Status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Installing,
    Broken,
    Available,
    Suspended,
}

/// Mirrors Archon.Servers.v0.Loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Loader {
    Forge,
    NeoForge,
    Fabric,
    Quilt,
    Purpur,
    Spigot,
    Vanilla,
    Paper,
}

/// Mirrors Archon.Servers.v0.Game.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Game {
    Minecraft,
}

/// Mirrors Archon.Servers.v0.UpstreamKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum UpstreamKind {
    Modpack,
    None,
}

/// The Modrinth pack a server was installed from. Mirrors Archon.Servers.v0.Upstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upstream {
    pub kind: UpstreamKind,
    pub version_id: String,
    pub project_id: String,
}

/// Where players join the server. Mirrors Archon.Servers.v0.Net.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Net {
    pub ip: Option<String>,
    pub port: u16,
    pub domain: String,
    pub outside: Option<OutsideAddress>,
}

/// Where the server is on the internet. Mirrors the `outside` object of Archon.Servers.v0.Net.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutsideAddress {
    pub ip: String,
    pub port: u16,
}

/// Mirrors the `state` union of Archon.Servers.v0.Reachability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ReachabilityState {
    Checking,
    Reachable,
    Unverified,
    Unreachable,
}

/// Whether a running server can be joined from the internet; `cause` says why not. Mirrors
/// Archon.Servers.v0.Reachability.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Reachability {
    pub state: ReachabilityState,
    pub cause: Option<String>,
}

/// Mirrors the Core-only `power_state` of Archon.Servers.v0.Server. `Offline`, `Crashed`, and
/// `Killed` say how the last run ended: stopped (or never run), crashed, or force-killed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerPowerState {
    Offline,
    Starting,
    Running,
    Stopping,
    Crashed,
    Killed,
}

/// The App instance a server follows. Mirrors Archon.Servers.v0.LinkedInstance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedInstance {
    pub id: String,
    pub name: String,
    pub state: LinkedPackState,
    pub error: Option<String>,
    pub visible_to_friends: bool,
    pub shared_instance_id: Option<String>,
    /// The version of the instance's share the server runs.
    pub version: Option<i64>,
}

/// Mirrors Archon.Servers.v0.Notice.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Notice {
    pub id: u32,
    pub dismissable: bool,
    pub title: String,
    pub message: String,
    pub level: String,
    pub announced: DateTime<Utc>,
}

/// Mirrors Archon.Servers.v0.NodeInfo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeInfo {
    pub token: String,
    pub instance: String,
}

/// Mirrors Archon.Servers.v0.Flows.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Flows {
    pub intro: bool,
}

/// Response of GET `/modrinth/v0/servers/{id}`, and an item of GET `/modrinth/v0/servers`.
/// `current_user_permissions` is a Modrinth scope bitset. `node` is this Core, as the host the
/// request reached, under `/servers/{id}` (see `files::auth`). The SFTP fields, `datacenter`,
/// `suspension_reason`, `notices`, and `is_medal` are Modrinth hosting fields Core fills with
/// constants. Mirrors Archon.Servers.v0.Server.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Server {
    pub server_id: Uuid,
    pub name: String,
    pub owner_id: String,
    pub net: Net,
    pub game: Game,
    pub backup_quota: u32,
    pub used_backup_quota: u32,
    pub status: ServerStatus,
    pub suspension_reason: Option<String>,
    pub loader: Option<Loader>,
    pub loader_version: Option<String>,
    pub mc_version: Option<String>,
    pub upstream: Option<Upstream>,
    pub sftp_username: String,
    pub sftp_password: String,
    pub sftp_host: String,
    pub datacenter: String,
    pub notices: Vec<Notice>,
    pub node: Option<NodeInfo>,
    pub flows: Flows,
    pub is_medal: bool,
    pub current_user_permissions: u64,
    pub locked_since: Option<DateTime<Utc>>,
    pub linked_instance: Option<LinkedInstance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub power_state: Option<ServerPowerState>,
    pub reachability: Option<Reachability>,
    /// Who is online, from Core's health check (see `servers::insight`). `None` while the
    /// server does not run.
    pub players: Option<Vec<String>>,
}

/// Response of GET `/modrinth/v0/servers/{id}/fs`: where the file API is and the token for it.
/// Mirrors Archon.Servers.v0.JWTAuth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JwtAuth {
    pub url: String,
    pub token: String,
}

/// Response of GET `/modrinth/v0/servers/{id}/ws`: the console socket and a ticket for it.
/// Mirrors Archon.Websocket.v0.WSAuth.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WsAuth {
    pub url: String,
    pub token: String,
}

/// Mirrors the `action` of `ArchonServersV0Module.power`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PowerAction {
    Start,
    Stop,
    Restart,
    Kill,
}

/// Body of POST `/modrinth/v0/servers/{id}/power`. Mirrors the body of
/// `ArchonServersV0Module.power`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PowerRequest {
    pub action: PowerAction,
}

/// Body of POST `/modrinth/v0/servers/{id}/name`. Mirrors the body of
/// `ArchonServersV0Module.updateName`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenameRequest {
    pub name: String,
}

/// Query of POST `/modrinth/v0/servers/{id}/reinstall`. `hard` erases the server's data first.
/// Mirrors the `hard` param of `ArchonServersV0Module.reinstall`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReinstallQuery {
    pub hard: bool,
}

/// Body of POST `/modrinth/v0/servers/{id}/reinstall`, told apart by which fields it has. Mirrors
/// Archon.Servers.v0.ReinstallRequest.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ReinstallRequest {
    /// Mirrors Archon.Servers.v0.ReinstallModpackRequest.
    Modpack {
        project_id: String,
        #[serde(default)]
        version_id: Option<String>,
    },
    /// Mirrors Archon.Servers.v0.ReinstallLoaderRequest.
    Loader {
        loader: String,
        #[serde(default)]
        loader_version: Option<String>,
        #[serde(default)]
        game_version: Option<String>,
    },
}

/// Response of GET `/v1/servers/{id}/worlds/{wid}/select-download`, and a world's
/// `download_method`. Mirrors Archon.Servers.v1.WorldDownloadMethod.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "method_type", rename_all = "snake_case")]
pub enum WorldDownloadMethod {
    DirectNodeDownload,
    Backup { backup_id: Uuid },
    Unavailable,
}

/// Response of GET `/v1/servers/{id}`, and an item of GET `/v1/servers`. `location` is this
/// Core, as `node` is above. `subdomain`, `specs`, the SFTP fields, and `tags` are Modrinth
/// hosting fields Core fills with constants. Mirrors Archon.Servers.v1.ServerFull.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerFull {
    pub id: Uuid,
    pub name: String,
    pub subdomain: String,
    pub specs: ServerResources,
    pub sftp_username: String,
    pub sftp_password: String,
    pub tags: Vec<String>,
    pub location: ServerLocation,
    pub worlds: Vec<WorldFull>,
}

/// Mirrors Archon.Servers.v1.ServerResources.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerResources {
    pub cpu: u32,
    pub memory_mb: u32,
    pub storage_mb: u64,
    pub swap_mb: u32,
}

/// Mirrors Archon.Servers.v1.ServerLocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ServerLocation {
    Assigned { location_metadata: LocationMetadata },
    Unassigned,
}

/// Mirrors the `location_metadata` object of Archon.Servers.v1.ServerLocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocationMetadata {
    pub region: String,
    pub region_should_be_user_displayed: bool,
    pub hostname: String,
    pub url_host: String,
    pub is_decommissioned_node: bool,
}

/// A server's world. Mirrors Archon.Servers.v1.WorldFull.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldFull {
    pub id: Uuid,
    pub name: String,
    pub created_at: DateTime<Utc>,
    pub is_active: bool,
    pub download_method: WorldDownloadMethod,
    /// Always `[]`: this is the removed backups API; the App reads backups from the queue.
    pub backups: [(); 0],
    pub content: Option<WorldContentInfo>,
    pub readiness: WorldReadiness,
}

/// Mirrors Archon.Servers.v1.WorldReadiness.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldReadiness {
    pub data_synchronized_fetched: bool,
}

/// Mirrors Archon.Servers.v1.WorldContentInfo.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldContentInfo {
    /// The share of a server that follows no instance, once it is shared (`api::share`). A
    /// server that follows an instance reads its instance's share and has none.
    pub shared_instance_id: Option<String>,
    /// Not tracked yet: Core does not compare the server to its share until a push asks.
    pub shared_instance_needs_update: bool,
    /// A push changes nothing the server runs, so it never needs a restart.
    pub requires_restart: bool,
    pub modloader: String,
    pub modloader_version: String,
    pub game_version: String,
    pub java_version: Option<u32>,
    pub invocation: Option<String>,
    pub original_invocation: Option<String>,
}

/// Body of POST `/local/servers`: create an unconfigured server, linked to `instance` when it is
/// created from an App instance. Mirrors the body of `ArchonCoresV1Module.createServer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServerRequest {
    pub name: String,
    #[serde(default)]
    pub instance: Option<CreateServerInstance>,
}

/// The App instance a new server follows. Mirrors the `instance` argument of
/// `ArchonCoresV1Module.createServer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServerInstance {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub visible_to_friends: Option<bool>,
    #[serde(default)]
    pub shared_instance_id: Option<String>,
}

/// Response of POST `/local/servers`. Mirrors the `{ id }` result of
/// `ArchonCoresV1Module.createServer`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateServerResponse {
    pub id: Uuid,
}

/// Body of PATCH `/local/servers/{id}/linked-instance`: the sharing settings to change. Mirrors
/// Archon.Servers.v0.LinkedInstanceSettings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinkedInstanceSettings {
    #[serde(default)]
    pub visible_to_friends: Option<bool>,
    #[serde(default)]
    pub shared_instance: Option<SharedInstance>,
}

/// `id` is null once the instance is no longer shared. Mirrors the `shared_instance` object of
/// Archon.Servers.v0.LinkedInstanceSettings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SharedInstance {
    pub id: Option<String>,
}

impl From<ServerError> for ApiError {
    fn from(error: ServerError) -> Self {
        let code = match &error {
            ServerError::NotFound(_) => ErrorCode::NotFound,
            ServerError::AlreadyRunning
            | ServerError::NotRunning
            | ServerError::Starting
            | ServerError::Installing
            | ServerError::BackingUp
            | ServerError::Unmanaged
            | ServerError::NoPorts
            | ServerError::Eula
            | ServerError::NotReady(_)
            | ServerError::Shim(_) => ErrorCode::Conflict,
            ServerError::StartFailed(_) | ServerError::Other(_) => {
                ErrorCode::Internal
            }
        };
        Self::new(code, format!("{error:#}"))
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(error: sqlx::Error) -> Self {
        ServerError::from(error).into()
    }
}

// Every route below checks the caller's role on the server, or that they own the Core.

/// The Modrinth scope bits a level reads as (`serverPermissionBits` in `@modrinth/ui`), so the
/// App shows each role only what it may do. The owner is `SERVER_ADMIN`; editors read, power,
/// console, files, setup, backups, and advanced settings; viewers read. Every value survives JSON
/// as a JavaScript number, which `u64::MAX` does not: it reads as 2^64, no bits at all.
fn permissions(level: Level) -> u64 {
    const BASE_READ: u64 = 1 << 63;
    match level {
        Level::Owner => !0 << 15,
        // BASE_READ down to ADVANCED, bits 63 to 57.
        Level::Editor => 0b111_1111 << 57,
        Level::Viewer => BASE_READ,
    }
}

/// The host a request reached Core at, which is where the App finds the server's node routes.
fn host(headers: &HeaderMap) -> &str {
    headers
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .unwrap_or("localhost")
}

/// What the hosting API shows for a server row, to a caller with `level` on it, reached at
/// `host`.
async fn server(
    state: &AppState,
    row: records::ServerRow,
    level: Level,
    host: &str,
) -> Result<Server, ApiError> {
    let power_state = match state.servers.phase(&row.id) {
        Some(Phase::Starting) => ServerPowerState::Starting,
        Some(Phase::Running | Phase::Unmanaged(_)) => ServerPowerState::Running,
        Some(Phase::Stopping) => ServerPowerState::Stopping,
        Some(Phase::Installing) => ServerPowerState::Offline,
        None => match records::last_outcome(&state.db, &row.id).await? {
            Some(records::Outcome::Crashed) => ServerPowerState::Crashed,
            Some(records::Outcome::Killed) => ServerPowerState::Killed,
            Some(records::Outcome::Stopped) | None => ServerPowerState::Offline,
        },
    };
    let owner_id: Option<String> =
        sqlx::query_scalar("SELECT owner_user_id FROM core WHERE id = 1")
            .fetch_optional(&state.db)
            .await?
            .flatten();
    Ok(Server {
        server_id: Uuid::parse_str(&row.id).unwrap_or_default(),
        name: row.name,
        owner_id: owner_id.unwrap_or_default(),
        net: Net {
            ip: None,
            port: u16::try_from(row.port).unwrap_or_default(),
            domain: String::new(),
            outside: None,
        },
        game: Game::Minecraft,
        backup_quota: 0,
        used_backup_quota: 0,
        status: if state.servers.phase(&row.id) == Some(Phase::Installing) {
            ServerStatus::Installing
        } else if row.game_version.is_some() {
            ServerStatus::Available
        } else {
            ServerStatus::Installing
        },
        suspension_reason: None,
        loader: row.loader.as_deref().and_then(loader),
        loader_version: row.loader_version,
        mc_version: row.game_version,
        upstream: None,
        sftp_username: String::new(),
        sftp_password: String::new(),
        sftp_host: String::new(),
        datacenter: String::new(),
        notices: Vec::new(),
        node: Some(NodeInfo {
            token: String::new(),
            instance: format!("{host}/servers/{}", row.id),
        }),
        flows: Flows { intro: false },
        is_medal: false,
        current_user_permissions: permissions(level),
        locked_since: None,
        linked_instance: linked_instance(state, &row.id).await?,
        power_state: Some(power_state),
        reachability: super::insight::archon(state, &row.id),
        players: state
            .servers
            .insight(&row.id)
            .map(|insight| insight.players.into_iter().collect()),
    })
}

/// The App instance a server follows, when its pack is a version of that instance's share.
async fn linked_instance(
    state: &AppState,
    id: &str,
) -> Result<Option<LinkedInstance>, ApiError> {
    let Some(pack) = content::records::pack(&state.db, id).await? else {
        return Ok(None);
    };
    let Some(linked) = content::install::linked(&pack) else {
        return Ok(None);
    };
    Ok(Some(LinkedInstance {
        id: linked.id,
        name: linked.name,
        state: super::content::linked_state(state, id, &pack).await?,
        error: pack.error,
        visible_to_friends: false,
        shared_instance_id: Some(linked.shared_instance_id),
        version: Some(linked.version),
    }))
}

/// `servers.loader` as the hosting API names it.
fn loader(stored: &str) -> Option<Loader> {
    Some(match stored {
        "vanilla" => Loader::Vanilla,
        "fabric" => Loader::Fabric,
        "quilt" => Loader::Quilt,
        "forge" => Loader::Forge,
        "neoforge" => Loader::NeoForge,
        "paper" => Loader::Paper,
        _ => return None,
    })
}

/// `GET /modrinth/v0/servers`: every server the caller has a role on, on one page.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<ServerGetResponse>, ApiError> {
    let mut servers = Vec::new();
    for row in records::list(&state.db).await? {
        if let Some(level) = caller.level(&state.db, &row.id).await? {
            servers.push(server(&state, row, level, host(&headers)).await?);
        }
    }
    let count = servers.len() as u32;
    Ok(Json(ServerGetResponse {
        servers,
        pagination: Pagination {
            current_page: 1,
            page_size: count,
            total_pages: 1,
            total_items: count,
        },
        users: BTreeMap::new(),
    }))
}

/// `GET /modrinth/v0/servers/{id}`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<Server>, ApiError> {
    let level = caller.require(&state.db, &id, Level::Viewer).await?;
    let row = records::get(&state.db, &id)
        .await?
        .ok_or(ServerError::NotFound(id))?;
    Ok(Json(server(&state, row, level, host(&headers)).await?))
}

/// `POST /local/servers`: an unconfigured server. Linked instances are step 4. Owner only.
pub async fn create(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateServerRequest>,
) -> Result<Json<CreateServerResponse>, ApiError> {
    caller.require_owner()?;
    let name = request.name.trim();
    if name.is_empty() || name.chars().count() > 64 {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "A server name has 1 to 64 characters.",
        ));
    }
    let row = state.servers.create(name).await?;
    activity::log(&state.db, &caller, Some(&row.id), Action::ServerCreated)
        .await;
    Ok(Json(CreateServerResponse {
        id: Uuid::parse_str(&row.id).unwrap_or_default(),
    }))
}

/// `POST /modrinth/v0/servers/{id}/power`. Start answers once Java runs; stop and kill once the
/// shim has the request; restart at once.
pub async fn power(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(request): Json<PowerRequest>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let action = match request.action {
        PowerAction::Start => Action::ServerStarted,
        PowerAction::Stop => Action::ServerStopped,
        PowerAction::Kill => Action::ServerKilled,
        PowerAction::Restart => Action::ServerRestarted,
    };
    match request.action {
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
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /modrinth/v0/servers/{id}/reinstall`: install a loader on a stopped server, as a bare
/// install through the content routes does, but answering once it is done. Never erases: `hard`
/// is refused.
pub async fn reinstall(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<ReinstallQuery>,
    Json(request): Json<ReinstallRequest>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let ReinstallRequest::Loader {
        loader,
        loader_version,
        game_version: Some(game_version),
    } = request
    else {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "Install packs through the content routes.",
        ));
    };
    let Some(loader) = ModLoader::parse(&loader).filter(|_| !query.hard) else {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "Core installs Vanilla, Fabric, Quilt, Forge, NeoForge, and Paper, and never erases a server.",
        ));
    };
    let busy = state.content.busy(&id)?;
    let hold = state.servers.hold(&id)?;
    content::records::unlink(&state.db, &id).await?;
    let progress = state.content.progress(&id);
    let result = content::install::loader(
        &state.content,
        &hold,
        &progress,
        loader,
        &game_version,
        loader_version.as_deref(),
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
    Ok(StatusCode::NO_CONTENT)
}

/// `POST /local/servers/{id}/eula`: the owner accepted the Minecraft EULA for this server.
pub async fn accept_eula(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    caller.require_owner()?;
    if !records::accept_eula(&state.db, &id).await? {
        return Err(ServerError::NotFound(id).into());
    }
    activity::log(&state.db, &caller, Some(&id), Action::EulaAccepted).await;
    Ok(StatusCode::NO_CONTENT)
}

/// `DELETE /local/servers/{id}`: delete a stopped server, its world, and every role on it. Owner
/// only, dangerous. Its activity history stays.
pub async fn delete(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    records::get(&state.db, &id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    if state.servers.hold_for_deletion(&id).is_err() {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "Stop the server before deleting it.",
        ));
    }
    if let Err(error) = super::share::drop_share(&state, &id).await {
        state.servers.release(&id);
        return Err(error);
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
    Ok(StatusCode::NO_CONTENT)
}

/// Delete a held server's row, and with it every role on it, owing the backend each removal.
/// Returns who had a role.
async fn remove_server(
    state: &AppState,
    caller: &Caller,
    id: &str,
) -> Result<Vec<String>, ApiError> {
    let mut tx = state.db.begin().await?;
    let members: Vec<String> =
        sqlx::query_scalar("SELECT user_id FROM access WHERE server_id = ?")
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
    sqlx::query("DELETE FROM servers WHERE id = ?")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    for user in &members {
        super::access::owe_removal(&mut tx, user, id).await?;
    }
    activity::record(
        &mut tx,
        caller.user_id(),
        Some(id),
        &Action::ServerDeleted,
    )
    .await?;
    tx.commit().await?;
    Ok(members)
}

/// `GET /modrinth/v0/servers/{id}/ws`: where the console socket is. The socket checks the
/// session itself, so the token here is empty. Editors and the owner only.
pub async fn ws_auth(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<WsAuth>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    let host = host(&headers);
    Ok(Json(WsAuth {
        url: format!("{host}/servers/{id}/ws"),
        token: String::new(),
    }))
}

/// The hosting API's full view of a server reached at `host`, with its one world, whose id is
/// the server's.
async fn server_full(
    state: &AppState,
    row: records::ServerRow,
    host: &str,
) -> Result<ServerFull, ApiError> {
    let created_at: i64 =
        sqlx::query_scalar("SELECT created_at FROM servers WHERE id = ?")
            .bind(&row.id)
            .fetch_one(&state.db)
            .await?;
    let created_at =
        DateTime::<Utc>::from_timestamp(created_at, 0).unwrap_or_default();
    let id = Uuid::parse_str(&row.id).unwrap_or_default();
    let paths = state.servers.paths();
    let invocation = launch::args(paths, &row)
        .ok()
        .map(|args| launch::invocation(&args));
    let follows_instance = content::records::pack(&state.db, &row.id)
        .await?
        .is_some_and(|pack| content::install::linked(&pack).is_some());
    let shared_instance_id = if follows_instance {
        None
    } else {
        content::records::share_id(&state.db, &row.id).await?
    };
    let content = match (&row.loader, &row.game_version) {
        (Some(loader), Some(game_version)) => Some(WorldContentInfo {
            shared_instance_id,
            shared_instance_needs_update: false,
            requires_restart: false,
            modloader: loader.clone(),
            modloader_version: row.loader_version.clone().unwrap_or_default(),
            game_version: game_version.clone(),
            java_version: launch::java_major(paths, &state.content.http, &row)
                .await
                .ok(),
            original_invocation: invocation.clone(),
            invocation,
        }),
        _ => None,
    };
    Ok(ServerFull {
        id,
        name: row.name.clone(),
        subdomain: String::new(),
        specs: ServerResources {
            cpu: std::thread::available_parallelism()
                .map_or(1, |cpus| cpus.get() as u32),
            memory_mb: u32::try_from(row.memory_mb).unwrap_or_default(),
            storage_mb: 0,
            swap_mb: 0,
        },
        sftp_username: String::new(),
        sftp_password: String::new(),
        tags: Vec::new(),
        location: ServerLocation::Assigned {
            location_metadata: LocationMetadata {
                region: String::new(),
                region_should_be_user_displayed: false,
                hostname: host.to_owned(),
                url_host: format!("{host}/servers/{}", row.id),
                is_decommissioned_node: false,
            },
        },
        worlds: vec![WorldFull {
            id,
            name: row.name,
            created_at,
            is_active: true,
            download_method: WorldDownloadMethod::DirectNodeDownload,
            backups: [],
            content,
            readiness: WorldReadiness {
                data_synchronized_fetched: true,
            },
        }],
    })
}

/// `GET /v1/servers`: every server the caller has a role on.
pub async fn list_v1(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Result<Json<Vec<ServerFull>>, ApiError> {
    let mut servers = Vec::new();
    for row in records::list(&state.db).await? {
        if caller.level(&state.db, &row.id).await?.is_some() {
            servers.push(server_full(&state, row, host(&headers)).await?);
        }
    }
    Ok(Json(servers))
}

/// `GET /v1/servers/{id}`.
pub async fn get_v1(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Path(id): Path<String>,
) -> Result<Json<ServerFull>, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
    let row = records::get(&state.db, &id)
        .await?
        .ok_or(ServerError::NotFound(id))?;
    Ok(Json(server_full(&state, row, host(&headers)).await?))
}
