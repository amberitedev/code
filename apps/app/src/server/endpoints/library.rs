use contracts::{Auth, Empty, Endpoint, Json, Method};
use engine::data::IconConfig;
use serde::{Deserialize, Serialize};

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
