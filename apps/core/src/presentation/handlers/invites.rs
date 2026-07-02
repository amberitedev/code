use crate::{
    application::{
        core_projection_service,
        invite_service::{self, CreateInvitationRequest},
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
    Ok(Json(json!(
        invite_service::create(&state, &claims.sub, body).await?
    )))
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
    Ok(Json(json!(
        invite_service::revoke(&state, &claims.sub, &id).await?
    )))
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
