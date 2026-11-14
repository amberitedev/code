use std::{
    path::{Component, Path},
    sync::Arc,
};

use axum::extract::State;
use contracts::{ApiError, Empty, ErrorCode, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{Edit, InstanceAccess},
    endpoints::logs::{
        ClearLiveLog, DeleteInstanceLog, GetInstanceLogOutput, GetLiveLog,
        ListInstanceLogs,
    },
    logs::{self, CensoredString, Logs},
};

pub async fn list_instance_logs(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<ListInstanceLogs>,
) -> Result<Json<Vec<Logs>>, ApiError> {
    logs::get_logs(&engine, &instance.id, request.query.clear_contents)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_instance_log_output(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<GetInstanceLogOutput>,
) -> Result<Json<CensoredString>, ApiError> {
    let filename = log_file_name(&request.params.filename)?;
    logs::get_output_by_filename(
        &engine,
        &instance.id,
        request.params.log_type,
        filename,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn delete_instance_log(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    request: Req<DeleteInstanceLog>,
) -> Result<Empty, ApiError> {
    let filename = log_file_name(&request.params.filename)?;
    logs::delete_logs_by_filename(
        &engine,
        &instance.id,
        request.params.log_type,
        filename,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn get_live_log(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<GetLiveLog>,
) -> Result<Json<CensoredString>, ApiError> {
    logs::get_live_log_buffer(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn clear_live_log(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<ClearLiveLog>,
) -> Result<Empty, ApiError> {
    logs::clear_live_log_buffer(&engine, &instance.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

/// The engine joins the name onto the instance's log folder, so it must not leave it.
fn log_file_name(filename: &str) -> Result<&str, ApiError> {
    let mut components = Path::new(filename).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(filename),
        _ => Err(ApiError::new(
            ErrorCode::InvalidInput,
            "a log is named by its file name",
        )),
    }
}
