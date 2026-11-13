use contracts::{Auth, Empty, Endpoint, Json, Method};
use engine::data::Settings;
use serde::{Deserialize, Serialize};

pub struct GetSettings;

impl Endpoint for GetSettings {
    const METHOD: Method = Method::Get;
    const PATH: &'static str = "/app/settings";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Json<AppSettings>;
}

/// Replaces every setting.
pub struct SetSettings;

impl Endpoint for SetSettings {
    const METHOD: Method = Method::Put;
    const PATH: &'static str = "/app/settings";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Json<AppSettings>;
    type Response = Empty;
}

/// Keeps the previous data folder after a move failed. It answers while the engine cannot start.
pub struct CancelDirectoryChange;

impl Endpoint for CancelDirectoryChange {
    const METHOD: Method = Method::Delete;
    const PATH: &'static str = "/app/settings/directory-change";
    const AUTH: Auth = Auth::None;
    type Params = ();
    type Query = ();
    type Body = Empty;
    type Response = Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct AppSettings(#[cfg_attr(test, ts(type = "Settings"))] pub Settings);
