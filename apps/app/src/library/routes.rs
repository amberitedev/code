use std::sync::Arc;

use axum::{Router, extract::State};
use contracts::{
    ApiError, Empty, Json,
    server::{Req, route},
};
use engine::Engine;

use crate::server::{
    AppState,
    endpoints::library::{
        ListHiddenInstances, ListRecentIconConfigs, RecentIconConfigs,
        SetInstanceHidden,
    },
};

pub fn routes() -> Router<AppState> {
    [
        route::<ListRecentIconConfigs, _, _, _>(list_recent_icon_configs),
        route::<ListHiddenInstances, _, _, _>(list_hidden_instances),
        route::<SetInstanceHidden, _, _, _>(set_instance_hidden),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
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

async fn list_hidden_instances(
    State(engine): State<Arc<Engine>>,
    _: Req<ListHiddenInstances>,
) -> Result<Json<Vec<String>>, ApiError> {
    engine::instance::get_hidden_instance_ids(&engine)
        .await
        .map(Json)
        .map_err(engine::routes::api_error)
}

async fn set_instance_hidden(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetInstanceHidden>,
) -> Result<Empty, ApiError> {
    engine::instance::set_instance_hidden(
        &engine,
        &body.instance_id,
        body.hidden,
    )
    .await
    .map(|()| Empty)
    .map_err(engine::routes::api_error)
}
