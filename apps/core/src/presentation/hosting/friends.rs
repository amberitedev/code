//! The "Visible to friends" setting of a server linked to an instance.
use super::{record, ApiResult};
use crate::{
    application::{state::AppState, visible_servers},
    presentation::{error::ApiError, extractors::AuthUser},
};
use axum::{
    extract::{Path, State},
    routing::patch,
    Json, Router,
};
use serde::Deserialize;
use serde_json::json;
use std::sync::Arc;

pub fn router() -> Router<Arc<AppState>> {
    Router::new().route(
        "/local/servers/:id/linked-instance",
        patch(set_linked_instance),
    )
}

#[derive(Deserialize)]
struct LinkedInstanceSettings {
    visible_to_friends: Option<bool>,
    /// The shared instance behind the linked instance, or `{"id": null}` once it is not shared.
    shared_instance: Option<SharedInstance>,
}

#[derive(Deserialize)]
struct SharedInstance {
    id: Option<String>,
}

/// Changes who sees a linked server. Parts left out of the body stay as they are.
async fn set_linked_instance(
    _user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<LinkedInstanceSettings>,
) -> ApiResult {
    let r = record(&state, &id).await?;
    let linked = visible_servers::set(
        &state,
        &r.id.to_string(),
        body.visible_to_friends,
        body.shared_instance.map(|shared| shared.id),
    )
    .await
    .map_err(|e| ApiError::Internal(e.to_string()))?;
    if !linked {
        return Err(ApiError::NotFound(
            "this server is not linked to an instance".into(),
        ));
    }
    Ok(Json(json!({})))
}
