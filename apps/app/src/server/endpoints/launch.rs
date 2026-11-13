use contracts::{Auth, Empty, Endpoint, Json, Method};
use serde::{Deserialize, Serialize};

/// Launches an instance as the active Minecraft account and joins a server.
pub struct JoinServer;

impl Endpoint for JoinServer {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/launch/server";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<ServerJoin>;
    type Response = Empty;
}

/// Launches an instance as the active Minecraft account and opens one of its worlds.
pub struct JoinSingleplayerWorld;

impl Endpoint for JoinSingleplayerWorld {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/launch/singleplayer";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<SingleplayerJoin>;
    type Response = Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct ServerJoin {
    pub instance_id: String,
    pub address: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SingleplayerJoin {
    pub instance_id: String,
    /// The world's folder in `saves`.
    pub world: String,
}
