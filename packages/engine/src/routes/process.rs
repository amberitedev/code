use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{EngineAccess, InstanceAccess, Owner, View},
    endpoints::process::{KillProcess, ListInstanceProcesses, ListProcesses},
    process,
    state::ProcessMetadata,
};

pub async fn list_processes(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    _: Req<ListProcesses>,
) -> Result<Json<Vec<ProcessMetadata>>, ApiError> {
    process::get_all(&engine).await.map(Json).map_err(api_error)
}

pub async fn list_instance_processes(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    _: Req<ListInstanceProcesses>,
) -> Result<Json<Vec<ProcessMetadata>>, ApiError> {
    process::get_by_instance_id(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn kill_process(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<KillProcess>,
) -> Result<Empty, ApiError> {
    process::kill(&engine, request.params.uuid)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
