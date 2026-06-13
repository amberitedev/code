use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{
        access_service, activity_service,
        social_models::{
            ActivityLogQuery, PatchAccessRequest, UpsertAccessRequest,
        },
        state::AppState,
    },
    domain::instance::InstanceId,
    presentation::{error::ApiError, extractors::AuthUser},
};

pub async fn list_core_access(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        access_service::list_core_access(&state, &claims.sub).await?
    )))
}

pub async fn grant_core_access(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpsertAccessRequest>,
) -> Result<Json<Value>, ApiError> {
    let target = body.user_id.clone();
    let member =
        access_service::upsert_core_access(&state, &claims.sub, body).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_granted",
        None,
        Some(&target),
        Some(json!({ "scope": "core", "role": member.role })),
    )
    .await?;
    Ok(Json(json!(member)))
}

pub async fn update_core_access(
    AuthUser(claims): AuthUser,
    Path(user_id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<PatchAccessRequest>,
) -> Result<Json<Value>, ApiError> {
    let member =
        access_service::patch_core_access(&state, &claims.sub, &user_id, body)
            .await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_updated",
        None,
        Some(&user_id),
        Some(json!({ "scope": "core", "role": member.role })),
    )
    .await?;
    Ok(Json(json!(member)))
}

pub async fn remove_core_access(
    AuthUser(claims): AuthUser,
    Path(user_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    access_service::remove_core_access(&state, &claims.sub, &user_id).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_removed",
        None,
        Some(&user_id),
        Some(json!({ "scope": "core" })),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_instance_access(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    parse_instance_id(&id)?;
    Ok(Json(json!(
        access_service::list_instance_access(&state, &claims.sub, &id).await?
    )))
}

pub async fn grant_instance_access(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpsertAccessRequest>,
) -> Result<Json<Value>, ApiError> {
    parse_instance_id(&id)?;
    let target = body.user_id.clone();
    let member =
        access_service::upsert_instance_access(&state, &claims.sub, &id, body)
            .await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_granted",
        Some(&id),
        Some(&target),
        Some(json!({ "scope": "instance", "role": member.role })),
    )
    .await?;
    Ok(Json(json!(member)))
}

pub async fn update_instance_access(
    AuthUser(claims): AuthUser,
    Path((id, user_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<PatchAccessRequest>,
) -> Result<Json<Value>, ApiError> {
    parse_instance_id(&id)?;
    let member = access_service::patch_instance_access(
        &state,
        &claims.sub,
        &id,
        &user_id,
        body,
    )
    .await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_updated",
        Some(&id),
        Some(&user_id),
        Some(json!({ "scope": "instance", "role": member.role })),
    )
    .await?;
    Ok(Json(json!(member)))
}

pub async fn remove_instance_access(
    AuthUser(claims): AuthUser,
    Path((id, user_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    parse_instance_id(&id)?;
    access_service::remove_instance_access(&state, &claims.sub, &id, &user_id)
        .await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_removed",
        Some(&id),
        Some(&user_id),
        Some(json!({ "scope": "instance" })),
    )
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_activity(
    AuthUser(claims): AuthUser,
    Query(query): Query<ActivityLogQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        activity_service::list_for_viewer(&state, &claims.sub, query).await?
    )))
}

pub async fn list_instance_activity(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    Query(mut query): Query<ActivityLogQuery>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    parse_instance_id(&id)?;
    access_service::require_instance_permission(
        &state,
        &claims.sub,
        &id,
        "server:view",
    )
    .await
    .map_err(|error| ApiError::Forbidden(error.to_string()))?;
    query.instance_id = Some(id);
    Ok(Json(json!(
        activity_service::list_for_viewer(&state, &claims.sub, query).await?
    )))
}

fn parse_instance_id(id: &str) -> Result<(), ApiError> {
    id.parse::<InstanceId>().map(|_| ()).map_err(|_| {
        ApiError::BadRequest("invalid instance id — must be a UUID".into())
    })
}
