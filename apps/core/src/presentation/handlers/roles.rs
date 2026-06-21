use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{
        role_service::{self, RoleSettingsRequest, SaveCoreRoleRequest},
        state::AppState,
    },
    presentation::{
        authz::require_core_manager, error::ApiError, extractors::AuthUser,
    },
};

pub async fn list(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    Ok(Json(json!(role_service::list(&state).await?)))
}

pub async fn save(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<SaveCoreRoleRequest>,
) -> Result<Json<Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    Ok(Json(json!(role_service::save(&state, body).await?)))
}

pub async fn retire(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    role_service::retire(&state, &id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn update_settings(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<RoleSettingsRequest>,
) -> Result<Json<Value>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    role_service::update_settings(&state, body).await?;
    Ok(Json(json!({ "ok": true })))
}
