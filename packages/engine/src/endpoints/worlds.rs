use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::{endpoints, instances::InstanceRef};
use crate::{
    instance::DesyncServerMode,
    worlds::{
        DisplayStatus, ProtocolVersion, ServerPackStatus, ServerStatus, World,
        WorldType, WorldWithInstance,
    },
};

endpoints! {
    /// The most recently played worlds across every instance, newest first.
    ListRecentWorlds: Get "/worlds/recent", (), RecentWorldsQuery, Empty => Json<Vec<WorldWithInstance>>;
    /// Pings a Minecraft server.
    GetServerStatus: Get "/server-status", (), ServerStatusQuery, Empty => Json<ServerStatus>;
    /// The instance's singleplayer worlds and servers.
    ListInstanceWorlds: Get "/instances/{id}/worlds", InstanceRef, (), Empty => Json<Vec<World>>;
    SetWorldDisplayStatus: Put "/instances/{id}/worlds/display-status", InstanceRef, (), Json<WorldDisplayStatusUpdate> => Empty;
    /// `null` until the instance is installed.
    GetInstanceProtocolVersion: Get "/instances/{id}/protocol-version", InstanceRef, (), Empty => Json<Option<ProtocolVersion>>;
    GetSingleplayerWorld: Get "/instances/{id}/saves/{world}", WorldSaveRef, (), Empty => Json<World>;
    DeleteWorld: Delete "/instances/{id}/saves/{world}", WorldSaveRef, (), Empty => Empty;
    RenameWorld: Put "/instances/{id}/saves/{world}/name", WorldSaveRef, (), Json<WorldRename> => Empty;
    ResetWorldIcon: Delete "/instances/{id}/saves/{world}/icon", WorldSaveRef, (), Empty => Empty;
    /// Zips the world into the instance's `backups` folder and returns the archive's size.
    BackupWorld: Post "/instances/{id}/saves/{world}/backups", WorldSaveRef, (), Empty => Json<u64>;
    /// Returns the new server's index.
    AddInstanceServer: Post "/instances/{id}/servers", InstanceRef, (), Json<NewInstanceServer> => Json<usize>;
    EnsureManagedInstanceServer: Put "/instances/{id}/servers/managed", InstanceRef, (), Json<ManagedInstanceServer> => Empty;
    DesyncInstanceServer: Post "/instances/{id}/servers/desync", InstanceRef, (), Json<InstanceServerDesync> => Empty;
    EditInstanceServer: Put "/instances/{id}/servers/{index}", InstanceServerRef, (), Json<InstanceServerUpdate> => Empty;
    RemoveInstanceServer: Delete "/instances/{id}/servers/{index}", InstanceServerRef, (), Empty => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct RecentWorldsQuery {
    pub limit: usize,
    /// A JSON array of statuses; every status when absent.
    pub display_statuses: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ServerStatusQuery {
    pub address: String,
    pub protocol_version: Option<u32>,
    pub legacy: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct WorldSaveRef {
    pub id: String,
    /// The world's folder in `saves`.
    pub world: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceServerRef {
    pub id: String,
    pub index: usize,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct WorldDisplayStatusUpdate {
    pub world_type: WorldType,
    /// A singleplayer world's folder or a server's address.
    pub world_id: String,
    pub display_status: DisplayStatus,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct WorldRename {
    pub name: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct NewInstanceServer {
    pub name: String,
    pub address: String,
    pub pack_status: ServerPackStatus,
    pub project_id: Option<String>,
    pub content_kind: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ManagedInstanceServer {
    pub name: String,
    pub address: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceServerUpdate {
    pub name: String,
    pub address: String,
    pub pack_status: ServerPackStatus,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceServerDesync {
    pub server_id: String,
    pub mode: DesyncServerMode,
}
