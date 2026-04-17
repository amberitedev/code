//! Instance API handlers - Start, stop, kill, command.

use axum::extract::Path;
use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use crate::application::registry::ServiceRegistry;
use crate::application::instance_service::InstanceServiceError;
use crate::domain::instances::InstanceId;
use crate::presentation::error::ApiError;

/// Start response.
#[derive(Serialize)]
pub struct StartResponse {
    pub status: String,
}

/// Handle start.
pub async fn start_instance(
    Path(instance_id): Path<Uuid>,
    State(registry): State<Arc<ServiceRegistry>>,
) -> Result<Json<StartResponse>, ApiError> {
    let id = InstanceId::new(instance_id);

    registry.instance_service.start_instance(id).await
        .map_err(|e| match e {
            InstanceServiceError::NotFound(_) => ApiError::not_found("Instance not found"),
            InstanceServiceError::AlreadyRunning => ApiError::bad_request("Already running"),
            _ => ApiError::internal("Unknown error"),
        })?;

    Ok(Json(StartResponse { status: "booting".into() }))
}

/// Stop response.
#[derive(Serialize)]
pub struct StopResponse {
    pub status: String,
}

/// Handle stop.
pub async fn stop_instance(
    Path(instance_id): Path<Uuid>,
    State(registry): State<Arc<ServiceRegistry>>,
) -> Result<Json<StopResponse>, ApiError> {
    let id = InstanceId::new(instance_id);

    registry.instance_service.stop_instance(id, true).await
        .map_err(|e| match e {
            InstanceServiceError::NotFound(_) => ApiError::not_found("Instance not found"),
            InstanceServiceError::NotRunning => ApiError::bad_request("Not running"),
            _ => ApiError::internal("Unknown error"),
        })?;

    Ok(Json(StopResponse { status: "stopping".into() }))
}

/// Kill response.
#[derive(Serialize)]
pub struct KillResponse {
    pub status: String,
}

/// Handle kill.
pub async fn kill_instance(
    Path(instance_id): Path<Uuid>,
    State(registry): State<Arc<ServiceRegistry>>,
) -> Result<Json<KillResponse>, ApiError> {
    let id = InstanceId::new(instance_id);

    registry.instance_service.kill_instance(id).await
        .map_err(|_| ApiError::not_found("Instance not found"))?;

    Ok(Json(KillResponse { status: "killed".into() }))
}

/// Command request.
#[derive(Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

/// Command response.
#[derive(Serialize)]
pub struct CommandResponse {
    pub status: String,
}

/// Handle command.
#[axum::debug_handler]
pub async fn send_command(
    Path(instance_id): Path<Uuid>,
    State(registry): State<Arc<ServiceRegistry>>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<CommandResponse>, ApiError> {
    let id = InstanceId(instance_id);

    registry.instance_service.send_command(id, &req.command).await
        .map_err(|_| ApiError::not_found("Instance not found"))?;

    Ok(Json(CommandResponse { status: "sent".into() }))
}