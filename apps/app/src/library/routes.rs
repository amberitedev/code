use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Json,
    server::{Req, route},
};
use engine::Engine;

use crate::server::{
    AppState,
    endpoints::library::{ListRecentIconConfigs, RecentIconConfigs},
};

pub fn routes() -> Router<AppState> {
    let (path, method) =
        route::<ListRecentIconConfigs, _, _, _>(list_recent_icon_configs);
    Router::new().route(path, method)
}

async fn list_recent_icon_configs(
    State(engine): State<Arc<Engine>>,
    _: Req<ListRecentIconConfigs>,
) -> Result<Json<RecentIconConfigs>, ApiError> {
    engine::instance::get_recent_icon_configs(&engine)
        .await
        .map(|configs| Json(RecentIconConfigs(configs)))
        .map_err(engine::routes::api_error)
}
