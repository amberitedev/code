use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        instance_service::{create_instance as svc_create_instance, CreateInstanceRequest},
        state::AppState,
    },
    domain::instance::{InstanceId, InstanceRecord, MemorySettings, ModLoader},
    presentation::{error::ApiError, extractors::AuthUser},
};

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub port: u16,
    pub memory: Option<MemorySettings>,
}

fn record_summary(r: &InstanceRecord) -> Value {
    json!({
        "id": r.id.to_string(), "name": r.name, "game_version": r.game_version,
        "loader": r.loader.to_string(), "loader_version": r.loader_version,
        "port": r.port, "memory": { "min_mb": r.memory.min_mb, "max_mb": r.memory.max_mb },
        "status": r.status.to_string(),
    })
}

fn record_detail(r: &InstanceRecord) -> Value {
    json!({
        "id": r.id.to_string(), "name": r.name, "game_version": r.game_version,
        "loader": r.loader.to_string(), "loader_version": r.loader_version,
        "port": r.port, "memory": { "min_mb": r.memory.min_mb, "max_mb": r.memory.max_mb },
        "java_version": r.java_version, "status": r.status.to_string(),
        "data_dir": r.data_dir, "created_at": r.created_at, "updated_at": r.updated_at,
    })
}

/// GET /instances — list all instances.
pub async fn list_instances(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let records = state.instance_store.list().await?;
    let instances: Vec<Value> = records.iter().map(record_summary).collect();
    Ok(Json(json!({ "instances": instances })))
}

/// GET /instances/:id — get a single instance.
pub async fn get_instance(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = id
        .parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id — must be a UUID".into()))?;
    let record = state.instance_store.get(&iid).await?;
    Ok(Json(record_detail(&record)))
}

/// POST /instances — create a new instance (JAR download is async).
pub async fn create_instance(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if body.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name cannot be empty".into()));
    }
    if body.port == 0 {
        return Err(ApiError::BadRequest("port cannot be 0".into()));
    }
    let req = CreateInstanceRequest {
        name: body.name,
        game_version: body.game_version,
        loader: body.loader,
        loader_version: body.loader_version,
        port: body.port,
        memory: body.memory.unwrap_or_default(),
    };
    let id = svc_create_instance(&state, req).await?;
    let record = state.instance_store.get(&id).await?;
    Ok((StatusCode::CREATED, Json(record_detail(&record))))
}

/// DELETE /instances/:id — delete an instance (must be offline).
pub async fn delete_instance(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    // SEC-04: reject non-UUID paths before touching the database.
    let iid = id
        .parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id — must be a UUID".into()))?;

    if state.instances.contains_key(&iid) {
        return Err(ApiError::Conflict(
            "stop the instance before deleting".into(),
        ));
    }

    state.instance_store.delete(&iid).await?;
    Ok(Json(json!({ "ok": true })))
}
