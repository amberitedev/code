use std::sync::Arc;

use axum::{
    Router,
    extract::{DefaultBodyLimit, State},
};
use contracts::{
    ApiError, Empty, Json,
    server::{Req, route},
};
use engine::Engine;

use crate::server::{
    AppState,
    endpoints::library::{
        CacheGeneratedIcon, CacheIcon, ListHiddenInstances,
        ListRecentIconConfigs, RecentIconConfigs, SetGeneratedInstanceIcon,
        SetInstanceHidden,
    },
};

pub fn routes() -> Router<AppState> {
    [
        route::<ListRecentIconConfigs, _, _, _>(list_recent_icon_configs),
        route::<ListHiddenInstances, _, _, _>(list_hidden_instances),
        route::<SetInstanceHidden, _, _, _>(set_instance_hidden),
        route::<CacheIcon, _, _, _>(cache_icon),
        route::<CacheGeneratedIcon, _, _, _>(cache_generated_icon),
        route::<SetGeneratedInstanceIcon, _, _, _>(set_generated_instance_icon),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
    // The engine caps icons and symbols at 4 MiB, past axum's default body limit.
    .layer(DefaultBodyLimit::disable())
}

async fn list_recent_icon_configs(
    State(engine): State<Arc<Engine>>,
    _: Req<ListRecentIconConfigs>,
) -> Result<Json<RecentIconConfigs>, ApiError> {
    super::recent_icons::list(crate::db::pool(&engine))
        .await
        .map(|configs| Json(RecentIconConfigs(configs)))
        .map_err(engine::routes::api_error)
}

async fn list_hidden_instances(
    State(engine): State<Arc<Engine>>,
    _: Req<ListHiddenInstances>,
) -> Result<Json<Vec<String>>, ApiError> {
    crate::library::get_hidden_instance_ids(&engine)
        .await
        .map(Json)
        .map_err(engine::routes::api_error)
}

async fn set_instance_hidden(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetInstanceHidden>,
) -> Result<Empty, ApiError> {
    crate::library::set_instance_hidden(&engine, &body.instance_id, body.hidden)
        .await
        .map(|()| Empty)
        .map_err(engine::routes::api_error)
}

async fn cache_icon(
    State(engine): State<Arc<Engine>>,
    request: Req<CacheIcon>,
) -> Result<Json<String>, ApiError> {
    engine::instance::cache_icon_bytes(&engine, request.body.bytes.to_vec())
        .await
        .map(Json)
        .map_err(engine::routes::api_error)
}

async fn cache_generated_icon(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<CacheGeneratedIcon>,
) -> Result<Json<String>, ApiError> {
    let icon = engine::instance::cache_generated_icon(
        &engine,
        body.config.clone(),
        body.symbol_bytes,
    )
    .await
    .map_err(engine::routes::api_error)?;
    if body.add_to_recents {
        super::recent_icons::record(&body.config, crate::db::pool(&engine))
            .await
            .map_err(engine::routes::api_error)?;
    }
    Ok(Json(icon))
}

async fn set_generated_instance_icon(
    State(engine): State<Arc<Engine>>,
    Req { params, body, .. }: Req<SetGeneratedInstanceIcon>,
) -> Result<Json<Option<String>>, ApiError> {
    let id = &params.id;
    let icon = if body.only_if_empty {
        engine::instance::edit_generated_icon_if_empty(
            &engine,
            id,
            body.config.clone(),
            body.symbol_bytes,
        )
        .await
    } else {
        engine::instance::edit_generated_icon(
            &engine,
            id,
            body.config.clone(),
            body.symbol_bytes,
        )
        .await
        .map(Some)
    }
    .map_err(engine::routes::api_error)?;
    if icon.is_some() {
        super::recent_icons::record(&body.config, crate::db::pool(&engine))
            .await
            .map_err(engine::routes::api_error)?;
    }
    Ok(Json(icon))
}
