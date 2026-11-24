use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Mount, Req},
};
use engine::{Engine, routes::api_error};

use crate::local_api::{
    AppState,
    endpoints::settings::{
        AppSettings, CancelDirectoryChange, GetSettings, SetSettings,
    },
};

pub fn routes() -> Router<AppState> {
    Router::new()
        .endpoint::<GetSettings, _, _>(get_settings)
        .endpoint::<SetSettings, _, _>(set_settings)
}

/// Routes that answer while the engine cannot start, so they wait on nothing.
pub fn startup_routes(identifier: String) -> Router {
    Router::new()
        .endpoint::<CancelDirectoryChange, _, _>(cancel_directory_change)
        .with_state(Arc::from(identifier))
}

async fn get_settings(
    State(engine): State<Arc<Engine>>,
    _: Req<GetSettings>,
) -> Result<Json<AppSettings>, ApiError> {
    super::get(&engine)
        .await
        .map(|settings| Json(AppSettings(settings)))
        .map_err(api_error)
}

async fn set_settings(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetSettings>,
) -> Result<Empty, ApiError> {
    super::set(&engine, body.0)
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
