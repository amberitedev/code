//! Instance API handlers - Start, stop, kill, command.

use axum::extract::{AuthExtractor, Path, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::sync::Arc;
use crate::application::registry::ServiceRegistry;
use crate::application::instance_service::InstanceServiceError;
use crate::domain::instances::InstanceId;
use crate::presentation::error::ApiError;
use crate::infrastructure::sqlite_repo::InstanceRow;

#[derive(Serialize)]
pub struct ListResponse {
    pub instances: Vec<InstanceResponse>,
}

#[derive(Serialize)]
pub struct InstanceResponse {
    pub id: String,
    pub name: String,
}

pub async fn list_instances(
    _auth: AuthExtractor,
    State(registry): State<Arc<ServiceRegistry>>,
) -> Result<Json<ListResponse>, ApiError> {
    let instances = registry.instance_service.list_instances().await
        .map_err(|_| ApiError::internal("Failed to list instances"))?;

    let response = ListResponse {
        instances: instances.into_iter().map(|i| InstanceResponse {
            id: i.id.0.to_string(),
            name: i.name,
        }).collect(),
    };

    Ok(Json(response))
}

#[derive(Serialize)]
pub struct StartResponse {
    pub status: String,
}

pub async fn start_instance(
    _auth: AuthExtractor,
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

#[derive(Serialize)]
pub struct StopResponse {
    pub status: String,
}

pub async fn stop_instance(
    _auth: AuthExtractor,
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

#[derive(Serialize)]
pub struct KillResponse {
    pub status: String,
}

pub async fn kill_instance(
    _auth: AuthExtractor,
    Path(instance_id): Path<Uuid>,
    State(registry): State<Arc<ServiceRegistry>>,
) -> Result<Json<KillResponse>, ApiError> {
    let id = InstanceId::new(instance_id);

    registry.instance_service.kill_instance(id).await
        .map_err(|_| ApiError::not_found("Instance not found"))?;

    Ok(Json(KillResponse { status: "killed".into() }))
}

#[derive(Deserialize)]
pub struct CommandRequest {
    pub command: String,
}

#[derive(Serialize)]
pub struct CommandResponse {
    pub status: String,
}

#[axum::debug_handler]
pub async fn send_command(
    _auth: AuthExtractor,
    Path(instance_id): Path<Uuid>,
    State(registry): State<Arc<ServiceRegistry>>,
    Json(req): Json<CommandRequest>,
) -> Result<Json<CommandResponse>, ApiError> {
    let id = InstanceId(instance_id);

    registry.instance_service.send_command(id, &req.command).await
        .map_err(|_| ApiError::not_found("Instance not found"))?;

    Ok(Json(CommandResponse { status: "sent".into() }))
}
