use std::sync::Arc;

use axum::{
    extract::{Path, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{social_models, social_service, state::AppState},
    presentation::{error::ApiError, extractors::AuthUser},
};

pub async fn get_core(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        social_service::get_core_metadata(&state).await?
    )))
}

pub async fn update_core(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<social_models::UpdateCoreMetadataRequest>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        social_service::update_core_metadata(&state, body).await?
    )))
}

pub async fn list_members(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({ "members": social_service::list_members(&state).await? }),
    ))
}

pub async fn upsert_member(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<social_models::UpsertMemberRequest>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        social_service::upsert_member(&state, body).await?
    )))
}

pub async fn remove_member(
    _auth: AuthUser,
    Path(user_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    social_service::remove_member(&state, &user_id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn ban_member(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<social_models::BanMemberRequest>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        social_service::ban_member(&state, body, &claims.sub).await?
    )))
}

pub async fn list_bans(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(
        json!({ "bans": social_service::list_bans(&state).await? }),
    ))
}
