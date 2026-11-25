use contracts::{Empty, Json, Raw};
use engine::data::IconConfig;
use engine::worlds::{World, WorldWithInstance};
use serde::{Deserialize, Serialize};

use super::IdParams;

contracts::endpoints! {
    group library;

    /// The icon configs this desktop picked last, newest first.
    ListRecentIconConfigs: Get "/app/library/recent-icon-configs", None, (), (), Empty => Json<RecentIconConfigs>;
    /// The ids of the instances the Library hides.
    ListHiddenInstances: Get "/app/library/hidden-instances", None, (), (), Empty => Json<Vec<String>>;
    SetInstanceHidden: Put "/app/library/hidden-instances", None, (), (), Json<InstanceVisibility> => Empty;
    /// Normalizes an image into this desktop's icon cache and answers the cached file's path.
    CacheIcon: Post "/app/library/icons", None, (), (), Raw => Json<String>;
    /// Draws an icon from the editor's settings into the icon cache and answers the cached file's path.
    CacheGeneratedIcon: Post "/app/library/icons/generated", None, (), (), Json<GeneratedIcon> => Json<String>;
    /// Draws an icon from the editor's settings and makes it the instance's. Answers the cached
    /// file's path, or `null` when `only_if_empty` kept the instance's own icon.
    SetGeneratedInstanceIcon: Put "/app/instances/{id}/icon/generated", None, IdParams, (), Json<GeneratedInstanceIcon> => Json<Option<String>>;
    /// The most recently played worlds across every instance, newest first.
    ListRecentWorlds: Get "/app/worlds/recent", None, (), RecentWorldsQuery, Empty => Json<Vec<WorldWithInstance>>;
    /// The instance's singleplayer worlds and servers.
    ListInstanceWorlds: Get "/app/instances/{id}/worlds", None, IdParams, (), Empty => Json<Vec<World>>;
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
pub struct GeneratedIcon {
    pub config: IconConfig,
    /// The symbol, a PNG.
    pub symbol_bytes: Vec<u8>,
    pub add_to_recents: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct GeneratedInstanceIcon {
    pub config: IconConfig,
    /// The symbol, a PNG.
    pub symbol_bytes: Vec<u8>,
    pub only_if_empty: bool,
}

#[derive(Serialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct RecentIconConfigs(
    #[cfg_attr(feature = "export-ts", ts(type = "Array<SharedIconConfig>"))]
    pub Vec<IconConfig>,
);

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct InstanceVisibility {
    pub instance_id: String,
    pub hidden: bool,
}
