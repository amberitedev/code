use contracts::{Empty, Json};
use engine::data::ProcessMetadata;
use serde::{Deserialize, Serialize};

contracts::endpoints! {
    group launch;

    /// Launches an instance as the active Minecraft account, joining `server_address` when set.
    PlayInstance: Post "/app/launch/instance", None, (), (), Json<InstancePlay> => Json<ProcessMetadata>;
    /// Launches an instance as the active Minecraft account and joins a server.
    JoinServer: Post "/app/launch/server", None, (), (), Json<ServerJoin> => Empty;
    /// Launches an instance as the active Minecraft account and opens one of its worlds.
    JoinSingleplayerWorld: Post "/app/launch/singleplayer", None, (), (), Json<SingleplayerJoin> => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstancePlay {
    pub instance_id: String,
    #[cfg_attr(feature = "export-ts", ts(optional = nullable))]
    pub server_address: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct ServerJoin {
    pub instance_id: String,
    pub address: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct SingleplayerJoin {
    pub instance_id: String,
    /// The world's folder in `saves`.
    pub world: String,
}
