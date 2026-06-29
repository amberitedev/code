use std::{collections::HashMap, path::PathBuf, sync::Arc};

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{instance_service, state::AppState},
    infrastructure::minecraft::server_properties::{
        patch_properties, read_properties,
    },
    presentation::{
        error::ApiError,
        extractors::AuthUser,
        instance_path::resolve_authorized_instance_id,
    },
};

/// GET /instances/:id/properties
pub async fn get_properties_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:settings",
    )
    .await?;
    let data_dir = fetch_data_dir(&state, &iid).await?;
    let props = read_properties(&data_dir)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Ok(Json(json!({ "properties": props })))
}

/// PATCH /instances/:id/properties — update specific keys in-place.
pub async fn patch_properties_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<HashMap<String, String>>,
) -> Result<Json<Value>, ApiError> {
    let iid = resolve_authorized_instance_id(
        &state,
        &claims.sub,
        &id,
        "server:settings",
    )
    .await?;
    let data_dir = fetch_data_dir(&state, &iid).await?;
    let body = normalize_properties(body);
    if let Some(port) = body.get("server-port") {
        let port = port.parse::<u16>().map_err(|_| {
            ApiError::BadRequest("server-port must be 1-65535".into())
        })?;
        instance_service::update_port(&state, &iid, port).await?;
    }

    let _updated = patch_properties(&data_dir, &body)
        .await
        .map_err(properties_error)?;
    Ok(Json(json!({ "ok": true })))
}

fn properties_error(
    error: crate::infrastructure::minecraft::server_properties::PropertiesError,
) -> ApiError {
    match error {
		crate::infrastructure::minecraft::server_properties::PropertiesError::Invalid(message) => {
			ApiError::BadRequest(message)
		}
		other => ApiError::Internal(other.to_string()),
	}
}

fn normalize_properties(
    body: HashMap<String, String>,
) -> HashMap<String, String> {
    body.into_iter()
        .map(|(key, value)| (normalize_key(&key).to_string(), value))
        .collect()
}

fn normalize_key(key: &str) -> &str {
    match key {
        "allow_flight" => "allow-flight",
        "difficulty" => "difficulty",
        "enable_command_block" => "enable-command-block",
        "enforce_whitelist" => "enforce-whitelist",
        "gamemode" => "gamemode",
        "hardcore" => "hardcore",
        "max_players" => "max-players",
        "motd" => "motd",
        "online_mode" => "online-mode",
        "pvp" => "pvp",
        "server_port" => "server-port",
        "spawn_protection" => "spawn-protection",
        "view_distance" => "view-distance",
        "white_list" => "white-list",
        _ => key,
    }
}

/// SEC-04: parse UUID and use instance_store — no raw SQL.
async fn fetch_data_dir(
    state: &Arc<AppState>,
    instance_id: &crate::domain::instance::InstanceId,
) -> Result<PathBuf, ApiError> {
    let record = state.instance_store.get(instance_id).await?;
    Ok(PathBuf::from(&record.data_dir))
}
