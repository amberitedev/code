use contracts::{Auth, Empty, Endpoint, Json, Method, Raw};
use engine::data::IconConfig;
use serde::{Deserialize, Serialize};

use super::IdParams;

/// The icon configs this desktop picked last, newest first.
pub struct ListRecentIconConfigs;

impl Endpoint for ListRecentIconConfigs {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/library/recent-icon-configs";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<RecentIconConfigs>;
}

/// The ids of the instances the Library hides.
pub struct ListHiddenInstances;

impl Endpoint for ListHiddenInstances {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/library/hidden-instances";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<Vec<String>>;
}

pub struct SetInstanceHidden;

impl Endpoint for SetInstanceHidden {
    const METHOD: Method = Method::Put;
    const PATH: &'static str = "/app/library/hidden-instances";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<InstanceVisibility>;
    type Response = Empty;
}

/// Normalizes an image into this desktop's icon cache and answers the cached file's path.
pub struct CacheIcon;

impl Endpoint for CacheIcon {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/library/icons";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Raw;
    type Response = Json<String>;
}

/// Draws an icon from the editor's settings into the icon cache and answers the cached file's path.
pub struct CacheGeneratedIcon;

impl Endpoint for CacheGeneratedIcon {
    const METHOD: Method = Method::Post;
    const PATH: &'static str = "/app/library/icons/generated";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<GeneratedIcon>;
    type Response = Json<String>;
}

/// Draws an icon from the editor's settings and makes it the instance's. Answers the cached
/// file's path, or `null` when `only_if_empty` kept the instance's own icon.
pub struct SetGeneratedInstanceIcon;

impl Endpoint for SetGeneratedInstanceIcon {
    const METHOD: Method = Method::Put;
    const PATH: &'static str = "/app/instances/{id}/icon/generated";
    const AUTH: Auth = Auth::None;
    type Params = IdParams;
    type Query = ();
    type Body = Json<GeneratedInstanceIcon>;
    type Response = Json<Option<String>>;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GeneratedIcon {
    pub config: IconConfig,
    /// The symbol, a PNG.
    pub symbol_bytes: Vec<u8>,
    pub add_to_recents: bool,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct GeneratedInstanceIcon {
    pub config: IconConfig,
    /// The symbol, a PNG.
    pub symbol_bytes: Vec<u8>,
    pub only_if_empty: bool,
}

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RecentIconConfigs(
    #[cfg_attr(test, ts(type = "Array<SharedIconConfig>"))] pub Vec<IconConfig>,
);

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct InstanceVisibility {
    pub instance_id: String,
    pub hidden: bool,
}
