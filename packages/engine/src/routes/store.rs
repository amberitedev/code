use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{EngineAccess, Owner},
    endpoints::store::{CleanStore, GetStoreUsage, SetStoreCacheLimit},
    settings::{self, StoreUsage},
};

pub async fn get_store_usage(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<GetStoreUsage>,
) -> Result<Json<StoreUsage>, ApiError> {
    settings::store_usage(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn clean_store(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<CleanStore>,
) -> Result<Json<u64>, ApiError> {
    settings::store_cleanup(&engine)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn set_store_cache_limit(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<SetStoreCacheLimit>,
) -> Result<Empty, ApiError> {
    settings::store_set_cache_limit(&engine, request.body)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
