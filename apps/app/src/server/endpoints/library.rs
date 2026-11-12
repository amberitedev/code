use contracts::{Auth, Empty, Endpoint, Json, Method};
use engine::data::IconConfig;
use serde::Serialize;

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

#[derive(Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct RecentIconConfigs(
    #[cfg_attr(test, ts(type = "Array<IconConfig>"))] pub Vec<IconConfig>,
);
