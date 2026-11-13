use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Req, route},
};
use engine::{Engine, routes::api_error, settings};

use crate::server::{
    AppState,
    endpoints::settings::{
        AppSettings, CancelDirectoryChange, GetSettings, SetSettings,
    },
};

pub fn routes() -> Router<AppState> {
    let (get_path, get) = route::<GetSettings, _, _, _>(get_settings);
    let (set_path, set) = route::<SetSettings, _, _, _>(set_settings);
    Router::new().route(get_path, get).route(set_path, set)
}

/// Routes that answer while the engine cannot start, so they wait on nothing.
pub fn startup_routes(identifier: String) -> Router {
    let (path, method) =
        route::<CancelDirectoryChange, _, _, _>(cancel_directory_change);
    Router::new()
        .route(path, method)
        .with_state(Arc::from(identifier))
}

async fn get_settings(
    State(engine): State<Arc<Engine>>,
    _: Req<GetSettings>,
) -> Result<Json<AppSettings>, ApiError> {
    settings::get(&engine)
        .await
        .map(|settings| Json(AppSettings(settings)))
        .map_err(api_error)
}

async fn set_settings(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetSettings>,
) -> Result<Empty, ApiError> {
    settings::set(&engine, body.0)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

async fn cancel_directory_change(
    State(identifier): State<Arc<str>>,
    _: Req<CancelDirectoryChange>,
) -> Result<Empty, ApiError> {
    crate::data_dir::cancel_directory_change(&identifier)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
