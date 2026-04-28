use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        instance_service::{create_instance as svc_create_instance, CreateInstanceRequest},
        state::AppState,
    },
    domain::instance::{InstanceId, MemorySettings, ModLoader},
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

/// GET /instances — list all instances.
pub async fn list_instances(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let rows: Vec<(String, String, String, String, Option<String>, i64, i64, i64, String)> =
        sqlx::query_as(
            "SELECT id, name, game_version, loader, loader_version, \
             port, memory_min, memory_max, status \
             FROM instances ORDER BY created_at DESC",
        )
        .fetch_all(&state.pool)
        .await?;

    let instances: Vec<Value> = rows
        .into_iter()
        .map(|(id, name, gv, loader, lv, port, min, max, status)| {
            json!({
                "id": id, "name": name, "game_version": gv,
                "loader": loader, "loader_version": lv, "port": port,
                "memory": { "min_mb": min, "max_mb": max },
                "status": status,
            })
        })
        .collect();

    Ok(Json(json!({ "instances": instances })))
}

/// GET /instances/:id — get a single instance.
pub async fn get_instance(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let row: Option<(
        String, String, String, String, Option<String>,
        i64, i64, i64, Option<i64>, String, String, String, String,
    )> = sqlx::query_as(
        "SELECT id, name, game_version, loader, loader_version, port, \
         memory_min, memory_max, java_version, status, data_dir, \
         created_at, updated_at FROM instances WHERE id = ?",
    )
    .bind(&id)
    .fetch_optional(&state.pool)
    .await?;

    let (id, name, gv, loader, lv, port, min, max, java, status, data_dir, created, updated) =
        row.ok_or_else(|| ApiError::NotFound(format!("instance {id} not found")))?;

    Ok(Json(json!({
        "id": id, "name": name, "game_version": gv,
        "loader": loader, "loader_version": lv, "port": port,
        "memory": { "min_mb": min, "max_mb": max },
        "java_version": java, "status": status, "data_dir": data_dir,
        "created_at": created, "updated_at": updated,
    })))
}

/// POST /instances — create a new instance (JAR download is async).
pub async fn create_instance(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<Json<Value>, ApiError> {
    let req = CreateInstanceRequest {
        name: body.name,
        game_version: body.game_version,
        loader: body.loader,
        loader_version: body.loader_version,
        port: body.port,
        memory: body.memory.unwrap_or_default(),
    };
    let id = svc_create_instance(&state, req).await?;
    Ok(Json(json!({ "id": id.to_string() })))
}

/// DELETE /instances/:id — delete an instance (must be offline).
pub async fn delete_instance(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    if let Ok(iid) = id.parse::<InstanceId>() {
        if state.instances.contains_key(&iid) {
            return Err(ApiError::Conflict(
                "stop the instance before deleting".into(),
            ));
        }
    }
    sqlx::query("DELETE FROM instances WHERE id = ?")
        .bind(&id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
