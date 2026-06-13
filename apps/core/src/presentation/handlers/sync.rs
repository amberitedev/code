use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Multipart, Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{
        access_service, social_lookup_service, social_models, social_service,
        social_sync_service, state::AppState, sync_archive_service,
        sync_query_service,
    },
    presentation::{
        authz::require_instance_permission, error::ApiError,
        extractors::AuthUser,
    },
};

pub async fn list_sync_profiles(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let mut profiles = Vec::new();
    for profile in social_service::list_sync_profiles(&state).await? {
        if can_access_sync_profile(&state, &claims.sub, &profile).await {
            profiles.push(profile);
        }
    }
    Ok(Json(json!({ "profiles": profiles })))
}

pub async fn register_sync_profile(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<social_models::RegisterSyncProfileRequest>,
) -> Result<Json<Value>, ApiError> {
    require_profile_target_permission(
        &state,
        &claims.sub,
        body.core_instance_id.as_deref(),
        "server:settings",
    )
    .await?;
    Ok(Json(json!(
        social_sync_service::register_sync_profile(&state, body).await?
    )))
}

pub async fn create_sync_profile_from_mrpack(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    let (archive, metadata, _) = parse_mrpack_multipart(&mut multipart).await?;
    let req =
        metadata.unwrap_or(social_models::CreateSyncProfileFromMrpackRequest {
            name: None,
            client_profile_id: None,
            core_instance_id: None,
            sync_enabled: None,
            notes: None,
        });
    require_profile_target_permission(
        &state,
        &claims.sub,
        req.core_instance_id.as_deref(),
        "server:content",
    )
    .await?;
    Ok(Json(json!(
        social_sync_service::create_profile_from_mrpack(
            &state,
            &claims.sub,
            req,
            archive
        )
        .await?
    )))
}

pub async fn remove_sync_profile(
    AuthUser(claims): AuthUser,
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_sync_profile_permission(
        &state,
        &claims.sub,
        &profile_id,
        "server:settings",
    )
    .await?;
    social_service::remove_sync_profile(&state, &profile_id).await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn publish_snapshot(
    AuthUser(claims): AuthUser,
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
    mut multipart: Multipart,
) -> Result<Json<Value>, ApiError> {
    require_sync_profile_permission(
        &state,
        &claims.sub,
        &profile_id,
        "server:content",
    )
    .await?;
    let (archive, _, notes) = parse_mrpack_multipart(&mut multipart).await?;
    Ok(Json(json!(
        social_sync_service::publish_snapshot(
            &state,
            &profile_id,
            &claims.sub,
            archive,
            notes
        )
        .await?
    )))
}

pub async fn check_sync_version(
    AuthUser(claims): AuthUser,
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_sync_profile_permission(
        &state,
        &claims.sub,
        &profile_id,
        "server:view",
    )
    .await?;
    Ok(Json(json!(
        social_sync_service::check_version(&state, &profile_id).await?
    )))
}

pub async fn download_snapshot(
    AuthUser(claims): AuthUser,
    Path((profile_id, snapshot_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    require_sync_profile_permission(
        &state,
        &claims.sub,
        &profile_id,
        "server:content",
    )
    .await?;
    social_sync_service::ensure_snapshot_in_profile(
        &state,
        &profile_id,
        &snapshot_id,
    )
    .await?;
    let path = sync_archive_service::archive_for_snapshot(&state, &snapshot_id)
        .await?;
    let data = tokio::fs::read(path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let disposition = format!("attachment; filename=\"{snapshot_id}.mrpack\"");
    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/zip"),
        )
        .header(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_str(&disposition)
                .unwrap_or(HeaderValue::from_static("attachment")),
        )
        .body(Body::from(data))
        .map_err(|e| ApiError::Internal(e.to_string()))?)
}

pub async fn list_snapshots(
    AuthUser(claims): AuthUser,
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_sync_profile_permission(
        &state,
        &claims.sub,
        &profile_id,
        "server:view",
    )
    .await?;
    Ok(Json(
        json!({ "snapshots": sync_query_service::list_snapshots(&state, &profile_id).await? }),
    ))
}

pub async fn list_sync_events(
    AuthUser(claims): AuthUser,
    Path(profile_id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    require_sync_profile_permission(
        &state,
        &claims.sub,
        &profile_id,
        "server:view",
    )
    .await?;
    Ok(Json(
        json!({ "events": sync_query_service::list_events(&state, &profile_id).await? }),
    ))
}

async fn can_access_sync_profile(
    state: &Arc<AppState>,
    user_id: &str,
    profile: &social_models::SyncProfile,
) -> bool {
    if let Some(instance_id) = profile.core_instance_id.as_deref() {
        access_service::require_instance_permission(
            state,
            user_id,
            instance_id,
            "server:view",
        )
        .await
        .is_ok()
    } else {
        access_service::require_core_manager(state, user_id)
            .await
            .is_ok()
    }
}

async fn require_sync_profile_permission(
    state: &Arc<AppState>,
    user_id: &str,
    profile_id: &str,
    permission: &str,
) -> Result<(), ApiError> {
    let profile =
        social_lookup_service::get_sync_profile(state, profile_id).await?;
    require_profile_target_permission(
        state,
        user_id,
        profile.core_instance_id.as_deref(),
        permission,
    )
    .await
}

async fn require_profile_target_permission(
    state: &Arc<AppState>,
    user_id: &str,
    core_instance_id: Option<&str>,
    permission: &str,
) -> Result<(), ApiError> {
    if let Some(instance_id) = core_instance_id {
        require_instance_permission(state, user_id, instance_id, permission)
            .await
    } else {
        access_service::require_core_manager(state, user_id)
            .await
            .map(|_| ())
            .map_err(|error| ApiError::Forbidden(error.to_string()))
    }
}

async fn parse_mrpack_multipart(
    multipart: &mut Multipart,
) -> Result<
    (
        bytes::Bytes,
        Option<social_models::CreateSyncProfileFromMrpackRequest>,
        Option<String>,
    ),
    ApiError,
> {
    let mut mrpack = None;
    let mut metadata = None;
    let mut notes = None;
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::BadRequest(e.to_string()))?
    {
        match field.name() {
            Some("mrpack") | Some("file") => {
                mrpack = Some(
                    field
                        .bytes()
                        .await
                        .map_err(|e| ApiError::BadRequest(e.to_string()))?,
                )
            }
            Some("metadata") => {
                let text = field
                    .text()
                    .await
                    .map_err(|e| ApiError::BadRequest(e.to_string()))?;
                metadata = Some(
                    serde_json::from_str(&text)
                        .map_err(|e| ApiError::BadRequest(e.to_string()))?,
                );
            }
            Some("notes") => {
                notes = Some(
                    field
                        .text()
                        .await
                        .map_err(|e| ApiError::BadRequest(e.to_string()))?,
                )
            }
            _ => {}
        }
    }
    let archive = mrpack
        .ok_or_else(|| ApiError::BadRequest("missing mrpack file".into()))?;
    Ok((archive, metadata, notes))
}
