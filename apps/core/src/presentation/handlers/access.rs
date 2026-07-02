use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{
        access_service, activity_service, core_projection_service,
        social_models::{
            ActivityLogQuery, PatchAccessRequest, UpsertAccessRequest,
        },
        state::AppState,
    },
    presentation::{
        error::ApiError,
        extractors::AuthUser,
        instance_path::{
            resolve_authorized_instance_id, resolve_instance_path,
        },
    },
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
    core_projection_service::sync_projection_best_effort(
        &state,
        "core-access-grant",
    )
    .await;
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
    core_projection_service::sync_projection_best_effort(
        &state,
        "core-access-update",
    )
    .await;
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
    core_projection_service::sync_projection_best_effort(
        &state,
        "core-access-remove",
    )
    .await;
    Ok(Json(json!({ "ok": true })))
}

pub async fn list_instance_access(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_instance_path(&state, &id).await?.id.to_string();
    Ok(Json(json!(
        access_service::list_instance_access(&state, &claims.sub, &instance_id)
            .await?
    )))
}

pub async fn grant_instance_access(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpsertAccessRequest>,
) -> Result<Json<Value>, ApiError> {
    let instance_id = resolve_instance_path(&state, &id).await?.id.to_string();
    let target = body.user_id.clone();
    let member = access_service::upsert_instance_access(
        &state,
        &claims.sub,
        &instance_id,
        body,
    )
    .await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_granted",
        Some(&instance_id),
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
    let instance_id = resolve_instance_path(&state, &id).await?.id.to_string();
    let member = access_service::patch_instance_access(
        &state,
        &claims.sub,
        &instance_id,
        &user_id,
        body,
    )
    .await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_updated",
        Some(&instance_id),
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
    let instance_id = resolve_instance_path(&state, &id).await?.id.to_string();
    access_service::remove_instance_access(
        &state,
        &claims.sub,
        &instance_id,
        &user_id,
    )
    .await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_removed",
        Some(&instance_id),
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
    let instance_id =
        resolve_authorized_instance_id(&state, &claims.sub, &id, "server:view")
            .await?
            .to_string();
    query.instance_id = Some(instance_id);
    Ok(Json(json!(
        activity_service::list_for_viewer(&state, &claims.sub, query).await?
    )))
}
