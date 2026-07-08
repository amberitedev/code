use crate::{
    application::{
        activity_service, core_projection_service,
        invite_service::{
            self, CreateInvitationRequest, UpdateInvitationRequest,
        },
        state::AppState,
    },
    presentation::{
        authz::require_core_manager, error::ApiError, extractors::AuthUser,
    },
};
use axum::{
    extract::{Path, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct ReviewRequest {
    accept: bool,
}
#[derive(Deserialize)]
pub struct ResponseRequest {
    accept: bool,
}
#[derive(Serialize)]
pub struct InvitationList {
    invitations: Vec<invite_service::CoreInvitation>,
}
pub async fn create(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateInvitationRequest>,
) -> Result<Json<Value>, ApiError> {
    let invitee_user_id = body.invitee_user_id.clone();
    let invitee_display_name = body.invitee_display_name.clone();
    let role_id = body.role_id.clone();
    let invitation = invite_service::create(&state, &claims.sub, body).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_invited",
        None,
        Some(&invitee_user_id),
        Some(json!({
            "user_id": invitee_user_id,
            "username": invitee_display_name,
            "role_id": role_id,
            "permissions": permissions_for_role_id(&role_id),
        })),
    )
    .await?;
    Ok(Json(json!(invitation)))
}
pub async fn list(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<InvitationList>, ApiError> {
    require_core_manager(&state, &claims.sub).await?;
    Ok(Json(InvitationList {
        invitations: invite_service::list(&state).await?,
    }))
}
pub async fn list_mine(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<InvitationList>, ApiError> {
    Ok(Json(InvitationList {
        invitations: invite_service::list_mine(&state, &claims.sub).await?,
    }))
}
pub async fn update(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateInvitationRequest>,
) -> Result<Json<Value>, ApiError> {
    let role_id = body.role_id.clone();
    let invitation =
        invite_service::update(&state, &claims.sub, &id, body).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_access_updated",
        None,
        Some(&invitation.invitee_user_id),
        Some(json!({
            "scope": "core",
            "user_id": invitation.invitee_user_id,
            "username": invitation.invitee_display_name,
            "role_id": role_id,
            "permissions": permissions_for_role_id(&role_id),
        })),
    )
    .await?;
    Ok(Json(json!(invitation)))
}
pub async fn review(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ReviewRequest>,
) -> Result<Json<Value>, ApiError> {
    Ok(Json(json!(
        invite_service::review(&state, &claims.sub, &id, body.accept).await?
    )))
}
pub async fn revoke(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let invitation = invite_service::revoke(&state, &claims.sub, &id).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "user_invite_revoked",
        None,
        Some(&invitation.invitee_user_id),
        Some(json!({
            "user_id": invitation.invitee_user_id,
            "username": invitation.invitee_display_name,
            "role_id": invitation.role_id,
        })),
    )
    .await?;
    Ok(Json(json!(invitation)))
}
pub async fn respond(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<ResponseRequest>,
) -> Result<Json<Value>, ApiError> {
    let accepted = body.accept;
    let invitation =
        invite_service::respond(&state, &claims.sub, &id, accepted).await?;
    if accepted {
        core_projection_service::sync_projection_best_effort(
            &state,
            "core-invite-accepted",
        )
        .await;
    }
    Ok(Json(json!(invitation)))
}

fn permissions_for_role_id(role_id: &str) -> &'static str {
    if role_id == "role-admin" {
        "BASE_READ | POWER_ACTIONS | FILES_WRITE"
    } else {
        "BASE_READ"
    }
}
