//! Modrinth Hosting's server access routes: the people on a server and its activity log.

use std::sync::Arc;

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    routing::{get, patch, post},
    Json, Router,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        access_service::{self, LogQuery, Role},
        state::AppState,
    },
    domain::{event::Event, instance::InstanceRecord},
    presentation::{
        error::ApiError,
        extractors::{CanManageUsers, CanRead, SignedIn},
        instance_path::resolve_instance_path,
    },
};

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/v1/servers/:id/users", get(list).post(add))
        .route("/v1/servers/:id/users/:user", patch(update).delete(remove))
        .route("/v1/servers/:id/users/:user/reinvite", post(reinvite))
        .route("/v1/servers/:id/invites/accept", post(accept))
        .route("/v1/servers/:id/invites/decline", post(leave))
        .route(
            "/v1/servers/:id/action-log",
            get(action_log).delete(clear_log),
        )
}

/// Make open Apps refetch this server, including who has access to it.
fn changed(state: &AppState, server: InstanceRecord) {
    state
        .broadcaster
        .send(Event::InstanceUpdated { instance: server });
}

async fn list(
    _user: CanRead,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<Value>, ApiError> {
    let server = resolve_instance_path(&state, &id).await?;
    Ok(Json(
        access_service::list(
            &state,
            &server.id.to_string(),
            &server.created_at.to_rfc3339(),
        )
        .await?,
    ))
}

#[derive(Deserialize)]
struct AddUser {
    /// An account id or a username.
    user_id: String,
    role: String,
}

async fn add(
    user: CanManageUsers,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<AddUser>,
) -> Result<StatusCode, ApiError> {
    let server = resolve_instance_path(&state, &id).await?;
    access_service::invite(
        &state,
        &user.0,
        &server.id.to_string(),
        &server.name,
        body.user_id.trim(),
        Role::assignable(&body.role)?,
    )
    .await?;
    changed(&state, server);
    Ok(StatusCode::NO_CONTENT)
}

/// The body is the role alone, as a JSON string.
async fn update(
    user: CanManageUsers,
    State(state): State<Arc<AppState>>,
    Path((id, target)): Path<(String, String)>,
    Json(role): Json<String>,
) -> Result<StatusCode, ApiError> {
    let server = resolve_instance_path(&state, &id).await?;
    access_service::set_role(
        &state,
        &user.0,
        &server.id.to_string(),
        &target,
        Role::assignable(&role)?,
    )
    .await?;
    changed(&state, server);
    Ok(StatusCode::NO_CONTENT)
}

async fn remove(
    user: CanManageUsers,
    State(state): State<Arc<AppState>>,
    Path((id, target)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let server = resolve_instance_path(&state, &id).await?;
    access_service::remove(&state, &user.0.id, &server.id.to_string(), &target)
        .await?;
    changed(&state, server);
    Ok(StatusCode::NO_CONTENT)
}

async fn reinvite(
    user: CanManageUsers,
    State(state): State<Arc<AppState>>,
    Path((id, target)): Path<(String, String)>,
) -> Result<Json<Value>, ApiError> {
    let server = resolve_instance_path(&state, &id).await?;
    let (sent, cooldown_seconds) = access_service::reinvite(
        &state,
        &user.0,
        &server.id.to_string(),
        &server.name,
        &target,
    )
    .await?;
    Ok(Json(
        json!({"sent": sent, "cooldown_seconds": cooldown_seconds}),
    ))
}

/// The invited account accepts its invite.
async fn accept(
    SignedIn(account): SignedIn,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    if !access_service::accept(&state, &account.id, &id).await? {
        return Err(ApiError::NotFound(
            "This invite is no longer available".into(),
        ));
    }
    changed(&state, resolve_instance_path(&state, &id).await?);
    Ok(StatusCode::NO_CONTENT)
}

/// The invited account declines its invite, or gives up the access it has.
async fn leave(
    SignedIn(account): SignedIn,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    access_service::remove(&state, &account.id, &id, &account.id).await?;
    changed(&state, resolve_instance_path(&state, &id).await?);
    Ok(StatusCode::NO_CONTENT)
}

async fn action_log(
    _user: CanRead,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<LogQuery>,
) -> Result<Json<Value>, ApiError> {
    let server = resolve_instance_path(&state, &id).await?;
    Ok(Json(
        access_service::read_log(&state, &server.id.to_string(), query).await?,
    ))
}

async fn clear_log(
    _user: CanManageUsers,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let server = resolve_instance_path(&state, &id).await?;
    access_service::clear_log(&state, &server.id.to_string()).await?;
    Ok(StatusCode::NO_CONTENT)
}
