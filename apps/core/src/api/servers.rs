//! Servers: `GET /modrinth/v0/servers`, `GET /modrinth/v0/servers/{id}` and its `power`, `name`,
//! `fs`, `ws`, and `reinstall` routes; `GET /v1/servers`, `GET /v1/servers/{id}`,
//! `GET /v1/servers/{id}/worlds/{wid}/select-download`, `DELETE /v1/servers/{id}/flows/intro`; and
//! Amberite's `POST /local/servers`, `DELETE /local/servers/{id}`, and
//! `PATCH /local/servers/{id}/linked-instance`. Mirrors `Archon.Servers` in
//! `packages/api-client/src/modules/archon/types.ts` (`archon/servers/v0.ts`, `archon/servers/v1.ts`,
//! and `createServer` in `archon/cores/v1.ts`).

#![allow(dead_code)] // Wired in step 2: server routes.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::content::LinkedPackState;

/// Query of GET `/modrinth/v0/servers`. Mirrors Archon.Servers.v0.GetServersOptions.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerListQuery {
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub offset: Option<u32>,
}

/// Response of GET `/modrinth/v0/servers`. `users` is keyed by user id. Mirrors
/// Archon.Servers.v0.ServerGetResponse.
#[derive(Debug, Clone, Serialize)]
pub struct ServerGetResponse {
    pub servers: Vec<Server>,
    pub pagination: Pagination,
    pub users: BTreeMap<String, ServerOwner>,
}

/// Mirrors Archon.Servers.v0.Pagination.
#[derive(Debug, Clone, Serialize)]
pub struct Pagination {
    pub current_page: u32,
    pub page_size: u32,
    pub total_pages: u32,
    pub total_items: u32,
}

/// Mirrors Archon.Servers.v0.ServerOwner.
#[derive(Debug, Clone, Serialize)]
pub struct ServerOwner {
    pub id: String,
    pub username: String,
    pub avatar_url: Option<String>,
}

/// Mirrors Archon.Servers.v0.Status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerStatus {
    Installing,
    Broken,
    Available,
    Suspended,
}

/// Mirrors Archon.Servers.v0.Loader.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
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
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum Game {
    Minecraft,
}

/// Mirrors Archon.Servers.v0.UpstreamKind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum UpstreamKind {
    Modpack,
    None,
}

/// The Modrinth pack a server was installed from. Mirrors Archon.Servers.v0.Upstream.
#[derive(Debug, Clone, Serialize)]
pub struct Upstream {
    pub kind: UpstreamKind,
    pub version_id: String,
    pub project_id: String,
}

/// Where players join the server. Mirrors Archon.Servers.v0.Net.
#[derive(Debug, Clone, Serialize)]
pub struct Net {
    pub ip: Option<String>,
    pub port: u16,
    pub domain: String,
    pub outside: Option<OutsideAddress>,
}

/// Where the server is on the internet. Mirrors the `outside` object of Archon.Servers.v0.Net.
#[derive(Debug, Clone, Serialize)]
pub struct OutsideAddress {
    pub ip: String,
    pub port: u16,
}

/// Mirrors the `state` union of Archon.Servers.v0.Reachability.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ReachabilityState {
    Checking,
    Reachable,
    Unverified,
    Unreachable,
}

/// Whether a running server can be joined from the internet; `cause` says why not. Mirrors
/// Archon.Servers.v0.Reachability.
#[derive(Debug, Clone, Serialize)]
pub struct Reachability {
    pub state: ReachabilityState,
    pub cause: Option<String>,
}

/// Mirrors the `power_state` union of Archon.Servers.v0.Server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ServerPowerState {
    Offline,
    Starting,
    Running,
    Stopping,
    Crashed,
}

/// The App instance a server follows. Mirrors Archon.Servers.v0.LinkedInstance.
#[derive(Debug, Clone, Serialize)]
pub struct LinkedInstance {
    pub id: String,
    pub name: String,
    pub state: LinkedPackState,
    pub error: Option<String>,
    pub visible_to_friends: bool,
    pub shared_instance_id: Option<String>,
}

/// Mirrors Archon.Servers.v0.Notice.
#[derive(Debug, Clone, Serialize)]
pub struct Notice {
    pub id: u32,
    pub dismissable: bool,
    pub title: String,
    pub message: String,
    pub level: String,
    pub announced: DateTime<Utc>,
}

/// Mirrors Archon.Servers.v0.NodeInfo.
#[derive(Debug, Clone, Serialize)]
pub struct NodeInfo {
    pub token: String,
    pub instance: String,
}

/// Mirrors Archon.Servers.v0.Flows.
#[derive(Debug, Clone, Serialize)]
pub struct Flows {
    pub intro: bool,
}

/// Response of GET `/modrinth/v0/servers/{id}`, and an item of GET `/modrinth/v0/servers`.
/// `current_user_permissions` is a Modrinth scope bitset. The SFTP fields, `datacenter`,
/// `suspension_reason`, `notices`, and `is_medal` are Modrinth hosting fields Core fills with
/// constants. Mirrors Archon.Servers.v0.Server.
#[derive(Debug, Clone, Serialize)]
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
}

/// Response of GET `/modrinth/v0/servers/{id}/fs`: where the file API is and the token for it.
/// Mirrors Archon.Servers.v0.JWTAuth.
#[derive(Debug, Clone, Serialize)]
pub struct JwtAuth {
    pub url: String,
    pub token: String,
}

/// Response of GET `/modrinth/v0/servers/{id}/ws`: the console socket and a ticket for it.
/// Mirrors Archon.Websocket.v0.WSAuth.
#[derive(Debug, Clone, Serialize)]
pub struct WsAuth {
    pub url: String,
    pub token: String,
}

/// Mirrors the `action` of `ArchonServersV0Module.power`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub enum PowerAction {
    Start,
    Stop,
    Restart,
    Kill,
}

/// Body of POST `/modrinth/v0/servers/{id}/power`. Mirrors the body of
/// `ArchonServersV0Module.power`.
#[derive(Debug, Clone, Deserialize)]
pub struct PowerRequest {
    pub action: PowerAction,
}

/// Body of POST `/modrinth/v0/servers/{id}/name`. Mirrors the body of
/// `ArchonServersV0Module.updateName`.
#[derive(Debug, Clone, Deserialize)]
pub struct RenameRequest {
    pub name: String,
}

/// Query of POST `/modrinth/v0/servers/{id}/reinstall`. `hard` erases the server's data first.
/// Mirrors the `hard` param of `ArchonServersV0Module.reinstall`.
#[derive(Debug, Clone, Deserialize)]
pub struct ReinstallQuery {
    pub hard: bool,
}

/// Body of POST `/modrinth/v0/servers/{id}/reinstall`, told apart by which fields it has. Mirrors
/// Archon.Servers.v0.ReinstallRequest.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "method_type", rename_all = "snake_case")]
pub enum WorldDownloadMethod {
    DirectNodeDownload,
    Backup { backup_id: Uuid },
    Unavailable,
}

/// Response of GET `/v1/servers/{id}`, and an item of GET `/v1/servers`. `subdomain`, `specs`,
/// the SFTP fields, `tags`, and `location` are Modrinth hosting fields Core fills with constants.
/// Mirrors Archon.Servers.v1.ServerFull.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct ServerResources {
    pub cpu: u32,
    pub memory_mb: u32,
    pub storage_mb: u64,
    pub swap_mb: u32,
}

/// Mirrors Archon.Servers.v1.ServerLocation.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum ServerLocation {
    Assigned { location_metadata: LocationMetadata },
    Unassigned,
}

/// Mirrors the `location_metadata` object of Archon.Servers.v1.ServerLocation.
#[derive(Debug, Clone, Serialize)]
pub struct LocationMetadata {
    pub region: String,
    pub region_should_be_user_displayed: bool,
    pub hostname: String,
    pub url_host: String,
    pub is_decommissioned_node: bool,
}

/// A server's world. Mirrors Archon.Servers.v1.WorldFull.
#[derive(Debug, Clone, Serialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct WorldReadiness {
    pub data_synchronized_fetched: bool,
}

/// Mirrors Archon.Servers.v1.WorldContentInfo.
#[derive(Debug, Clone, Serialize)]
pub struct WorldContentInfo {
    pub modloader: String,
    pub modloader_version: String,
    pub game_version: String,
    pub java_version: Option<u32>,
    pub invocation: Option<String>,
    pub original_invocation: Option<String>,
}

/// Body of POST `/local/servers`: create an unconfigured server, linked to `instance` when it is
/// created from an App instance. Mirrors the body of `ArchonCoresV1Module.createServer`.
#[derive(Debug, Clone, Deserialize)]
pub struct CreateServerRequest {
    pub name: String,
    #[serde(default)]
    pub instance: Option<CreateServerInstance>,
}

/// The App instance a new server follows. Mirrors the `instance` argument of
/// `ArchonCoresV1Module.createServer`.
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug, Clone, Serialize)]
pub struct CreateServerResponse {
    pub id: Uuid,
}

/// Body of PATCH `/local/servers/{id}/linked-instance`: the sharing settings to change. Mirrors
/// Archon.Servers.v0.LinkedInstanceSettings.
#[derive(Debug, Clone, Deserialize)]
pub struct LinkedInstanceSettings {
    #[serde(default)]
    pub visible_to_friends: Option<bool>,
    #[serde(default)]
    pub shared_instance: Option<SharedInstance>,
}

/// `id` is null once the instance is no longer shared. Mirrors the `shared_instance` object of
/// Archon.Servers.v0.LinkedInstanceSettings.
#[derive(Debug, Clone, Deserialize)]
pub struct SharedInstance {
    pub id: Option<String>,
}
