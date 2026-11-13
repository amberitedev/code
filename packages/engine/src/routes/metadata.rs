use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{EngineAccess, Owner},
    endpoints::metadata::GetLoaderManifest,
    metadata::{self, Manifest},
};

pub async fn get_loader_manifest(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<GetLoaderManifest>,
) -> Result<Json<Manifest>, ApiError> {
    metadata::get_loader_versions(&engine, &request.params.loader)
        .await
        .map(Json)
        .map_err(api_error)
}
