use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::instances::InstanceRef;
use crate::{
    server_list::DesyncServerMode,
    worlds::{
        DisplayStatus, ProtocolVersion, ServerPackStatus, ServerStatus, World,
        WorldType, WorldWithInstance,
    },
};

contracts::endpoints! {
    group worlds;

    /// The most recently played worlds across every instance, newest first.
    ListRecentWorlds: Get "/worlds/recent", Session, (), RecentWorldsQuery, Empty => Json<Vec<WorldWithInstance>>, client_only;
    /// Pings a Minecraft server.
    GetServerStatus: Get "/server-status", Session, (), ServerStatusQuery, Empty => Json<ServerStatus>, client_only;
    /// The instance's singleplayer worlds and servers.
    ListInstanceWorlds: Get "/instances/{id}/worlds", Session, InstanceRef, (), Empty => Json<Vec<World>>, client_only;
    SetWorldDisplayStatus: Put "/instances/{id}/worlds/display-status", Session, InstanceRef, (), Json<WorldDisplayStatusUpdate> => Empty, client_only;
    /// `null` until the instance is installed.
    GetInstanceProtocolVersion: Get "/instances/{id}/protocol-version", Session, InstanceRef, (), Empty => Json<Option<ProtocolVersion>>, client_only;
    GetSingleplayerWorld: Get "/instances/{id}/saves/{world}", Session, WorldSaveRef, (), Empty => Json<World>, client_only;
    DeleteWorld: Delete "/instances/{id}/saves/{world}", Session, WorldSaveRef, (), Empty => Empty, client_only;
    RenameWorld: Put "/instances/{id}/saves/{world}/name", Session, WorldSaveRef, (), Json<WorldRename> => Empty, client_only;
    ResetWorldIcon: Delete "/instances/{id}/saves/{world}/icon", Session, WorldSaveRef, (), Empty => Empty, client_only;
    /// Zips the world into the instance's `backups` folder and returns the archive's size.
    BackupWorld: Post "/instances/{id}/saves/{world}/backups", Session, WorldSaveRef, (), Empty => Json<u64>, client_only;
    /// Returns the new server's index.
    AddInstanceServer: Post "/instances/{id}/servers", Session, InstanceRef, (), Json<NewInstanceServer> => Json<usize>, client_only;
    EnsureManagedInstanceServer: Put "/instances/{id}/servers/managed", Session, InstanceRef, (), Json<ManagedInstanceServer> => Empty, client_only;
    DesyncInstanceServer: Post "/instances/{id}/servers/desync", Session, InstanceRef, (), Json<InstanceServerDesync> => Empty, client_only;
    EditInstanceServer: Put "/instances/{id}/servers/{index}", Session, InstanceServerRef, (), Json<InstanceServerUpdate> => Empty, client_only;
    RemoveInstanceServer: Delete "/instances/{id}/servers/{index}", Session, InstanceServerRef, (), Empty => Empty, client_only;
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
