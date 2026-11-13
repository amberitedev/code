use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{Access, Caller, InstanceAccess, Permission, View},
    endpoints::process::{KillProcess, ListInstanceProcesses, ListProcesses},
    process,
    state::ProcessMetadata,
};

pub async fn list_processes(
    State(engine): State<Arc<Engine>>,
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    _: Req<ListProcesses>,
) -> Result<Json<Vec<ProcessMetadata>>, ApiError> {
    let mut visible = Vec::new();
    for metadata in process::get_all(&engine).await.map_err(api_error)? {
        if access
            .require(&caller, Some(&metadata.instance_id), Permission::View)
            .await
            .is_ok()
        {
            visible.push(metadata);
        }
    }
    Ok(Json(visible))
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
    State(access): State<Arc<dyn Access>>,
    caller: Caller,
    request: Req<KillProcess>,
) -> Result<Empty, ApiError> {
    let uuid = request.params.uuid;
    let running = process::get_all(&engine).await.map_err(api_error)?;
    let Some(metadata) = running.into_iter().find(|it| it.uuid == uuid) else {
        return Ok(Empty);
    };
    access
        .require(&caller, Some(&metadata.instance_id), Permission::Edit)
        .await?;
    process::kill(&engine, uuid)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
