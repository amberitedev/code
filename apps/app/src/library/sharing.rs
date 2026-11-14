use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{Router, extract::State, http::StatusCode};
use contracts::{
    ApiError, Empty, Json,
    server::{Req, route},
};
use engine::{Engine, data::CachedSharedInstanceSummary, routes::api_error};

use crate::server::{
    AppState,
    endpoints::sharing::{
        CanUseSharedInstances, GetCachedSharedLibrary, PublishSharedInstance,
        SetCachedSharedLibrary,
    },
    ops::Ops,
};

pub fn routes() -> Router<AppState> {
    [
        route::<CanUseSharedInstances, _, _, _>(can_use_shared_instances),
        route::<GetCachedSharedLibrary, _, _, _>(get_cached_shared_library),
        route::<SetCachedSharedLibrary, _, _, _>(set_cached_shared_library),
        route::<PublishSharedInstance, _, _, _>(publish_shared_instance),
    ]
    .into_iter()
    .fold(Router::new(), |router, (path, method)| {
        router.route(path, method)
    })
}

async fn can_use_shared_instances(
    State(engine): State<Arc<Engine>>,
    _: Req<CanUseSharedInstances>,
) -> Result<Json<bool>, ApiError> {
    engine::instance::can_active_user_use_shared_instances(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn get_cached_shared_library(
    State(engine): State<Arc<Engine>>,
    _: Req<GetCachedSharedLibrary>,
) -> Result<Json<Vec<CachedSharedInstanceSummary>>, ApiError> {
    let Some(user_id) =
        crate::accounts::active_modrinth_user_id(crate::db::pool(&engine))
            .await
            .map_err(api_error)?
    else {
        return Ok(Json(Vec::new()));
    };
    engine::instance::get_cached_shared_instance_library(&engine, &user_id)
        .await
        .map(Json)
        .map_err(api_error)
}

async fn set_cached_shared_library(
    State(engine): State<Arc<Engine>>,
    Req { body, .. }: Req<SetCachedSharedLibrary>,
) -> Result<Empty, ApiError> {
    let Some(user_id) =
        crate::accounts::active_modrinth_user_id(crate::db::pool(&engine))
            .await
            .map_err(api_error)?
    else {
        return Ok(Empty);
    };
    engine::instance::cache_shared_instance_library(&engine, &user_id, body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

/// Runs to its end even when cancelled: stopping midway would leave the share applying.
async fn publish_shared_instance(
    State(engine): State<Arc<Engine>>,
    State(ops): State<Arc<Ops>>,
    Req { body, .. }: Req<PublishSharedInstance>,
) -> Result<StatusCode, ApiError> {
    ops.start(body.op, |op| async move {
        let last_progress = Mutex::new(None::<Instant>);
        let progress = |uploaded: u64, total: u64| {
            let Ok(mut last) = last_progress.lock() else {
                return;
            };
            if uploaded == total
                || last.is_none_or(|last| {
                    last.elapsed() >= Duration::from_millis(100)
                })
            {
                op.progress(None, uploaded, Some(total));
                *last = Some(Instant::now());
            }
        };
        engine::instance::publish_shared_instance(
            &engine,
            &body.instance_id,
            body.file_paths,
            body.removed_paths,
            body.server_core_id,
            &progress,
        )
        .await
        .map(|_| None)
        .map_err(api_error)
    })
}
