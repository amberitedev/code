//! Linked server source endpoints. Thin wrappers over `server_source_service` so they can move
//! under the hosting router later. All require `server:content`.

use std::{collections::HashMap, sync::Arc};

use axum::{
    extract::{Path, State},
    Json,
};
use serde::Deserialize;

use crate::{
    application::{
        server_source_service::{self, LinkRequest},
        state::AppState,
    },
    domain::server_source::{ServerContent, ServerSource, ServerSourceStatus},
    presentation::{
        error::ApiError, extractors::AuthUser,
        instance_path::resolve_authorized_instance_id,
    },
};

/// `CoreLinkSourceBody`
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum LinkBody {
    Instance {
        shared_instance_id: String,
        version: u64,
        sharing_url: String,
        server_token: String,
        #[serde(default)]
        properties: Option<Properties>,
    },
    Modrinth {
        project_id: String,
        version_id: String,
    },
}

/// `Archon.Content.v1.PropertiesFields`. Null values select the Minecraft default.
#[derive(Deserialize, Default)]
pub struct Properties {
    #[serde(default)]
    known: HashMap<String, Option<String>>,
    #[serde(default)]
    custom: HashMap<String, Option<String>>,
}

/// `CoreSourceUpdateBody`
#[derive(Deserialize)]
pub struct UpdateBody {
    version: String,
    restart: bool,
}

async fn authorize(
    state: &Arc<AppState>,
    user: &AuthUser,
    id: &str,
) -> Result<String, ApiError> {
    Ok(
        resolve_authorized_instance_id(state, &user.0.sub, id, "server:content")
            .await?
            .to_string(),
    )
}

/// GET /instances/:id/source
pub async fn get_source(
    user: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Option<ServerSourceStatus>>, ApiError> {
    let id = authorize(&state, &user, &id).await?;
    Ok(Json(server_source_service::get_status(&state, &id).await?))
}

/// POST /instances/:id/source
pub async fn link_source(
    user: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<LinkBody>,
) -> Result<Json<ServerSourceStatus>, ApiError> {
    let id = authorize(&state, &user, &id).await?;
    let req = match body {
        LinkBody::Instance {
            shared_instance_id,
            version,
            sharing_url,
            server_token,
            properties,
        } => LinkRequest {
            source: ServerSource::Instance {
                shared_instance_id,
                name: String::new(),
                icon: None,
            },
            version: version.to_string(),
            sharing: Some((sharing_url, server_token)),
            properties: properties.map(property_updates).unwrap_or_default(),
        },
        LinkBody::Modrinth {
            project_id,
            version_id,
        } => LinkRequest {
            source: ServerSource::Modrinth {
                project_id,
                version_id: version_id.clone(),
            },
            version: version_id,
            sharing: None,
            properties: HashMap::new(),
        },
    };
    Ok(Json(server_source_service::link(&state, &id, req).await?))
}

/// DELETE /instances/:id/source
pub async fn unlink_source(
    user: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<serde_json::Value>, ApiError> {
    let id = authorize(&state, &user, &id).await?;
    server_source_service::unlink(&state, &id).await?;
    Ok(Json(serde_json::json!({ "ok": true })))
}

/// POST /instances/:id/source/update
pub async fn update_source(
    user: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateBody>,
) -> Result<Json<ServerSourceStatus>, ApiError> {
    let id = authorize(&state, &user, &id).await?;
    Ok(Json(
        server_source_service::request_source_update(
            &state,
            &id,
            &body.version,
            body.restart,
        )
        .await?,
    ))
}

/// GET /instances/:id/content
pub async fn get_content(
    user: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<ServerContent>, ApiError> {
    let id = authorize(&state, &user, &id).await?;
    Ok(Json(server_source_service::content(&state, &id).await?))
}

/// Archon property names use underscores; server.properties uses dashes, as in the hosting
/// onboarding path.
fn property_updates(props: Properties) -> HashMap<String, String> {
    props
        .known
        .into_iter()
        .map(|(key, value)| (key.replace('_', "-"), value))
        .chain(props.custom)
        .filter_map(|(key, value)| Some((key, value?)))
        .collect()
}
