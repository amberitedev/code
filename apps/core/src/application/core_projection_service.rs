use std::sync::Arc;

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};

use crate::application::state::AppState;

#[derive(Debug, Error)]
pub enum ProjectionSyncError {
    #[error("Core is not paired with a Convex projection credential")]
    MissingCredential,
    #[error(transparent)]
    Database(#[from] sqlx::Error),
    #[error(transparent)]
    Http(#[from] reqwest::Error),
    #[error("Convex projection sync failed: {0}")]
    Rejected(String),
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionSnapshot {
    core_id: String,
    owner_user_id: String,
    link_state: &'static str,
    connection_url: Option<String>,
    setup_mode: Option<String>,
    members: Vec<ProjectionMember>,
    revision: i64,
    last_seen_at: i64,
    synced_at: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ProjectionMember {
    user_id: String,
    is_owner: bool,
}

#[derive(Debug, sqlx::FromRow)]
struct CoreConfigRow {
    owner_user_id: String,
    sync_credential: Option<String>,
}

#[derive(Debug, sqlx::FromRow)]
struct CoreMemberRow {
    user_id: String,
    role: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectionSyncResult {
    pub ok: bool,
    pub status: String,
    pub core_id: Option<String>,
    pub projection_revision: Option<i64>,
    pub synced_at: Option<i64>,
}

pub async fn sync_projection(
    state: &Arc<AppState>,
) -> Result<ProjectionSyncResult, ProjectionSyncError> {
    let config = sqlx::query_as::<_, CoreConfigRow>(
        "SELECT owner_user_id, realtime_credential AS sync_credential FROM core_config WHERE id = 1",
    )
	.fetch_optional(&state.pool)
	.await?
	.ok_or(ProjectionSyncError::MissingCredential)?;
    let credential = config
        .sync_credential
        .clone()
        .ok_or(ProjectionSyncError::MissingCredential)?;
    let snapshot = projection_snapshot(state, &config).await?;
    let endpoint = format!(
        "{}/core/projection-sync",
        state.config.convex_site_url.trim_end_matches('/')
    );
    let response = state
        .http
        .post(endpoint)
        .bearer_auth(credential)
        .json(&snapshot)
        .send()
        .await?;
    let status = response.status();
    if !status.is_success() {
        return Err(ProjectionSyncError::Rejected(status.to_string()));
    }
    Ok(response.json().await?)
}

pub async fn sync_projection_best_effort(state: &Arc<AppState>, reason: &str) {
    match sync_projection(state).await {
        Ok(result) => {
            info!(
                reason,
                status = %result.status,
                core_id = ?result.core_id,
                "synced Core projection to Convex"
            );
        }
        Err(error) => {
            warn!(reason, %error, "Core projection sync failed");
        }
    }
}

async fn projection_snapshot(
    state: &Arc<AppState>,
    config: &CoreConfigRow,
) -> Result<ProjectionSnapshot, sqlx::Error> {
    let setup_mode: Option<String> =
        sqlx::query_scalar("SELECT setup_mode FROM core_metadata WHERE id = 1")
            .fetch_optional(&state.pool)
            .await?;
    let members: Vec<CoreMemberRow> = sqlx::query_as(
		"SELECT user_id, role FROM core_members WHERE status = 'active' ORDER BY user_id",
	)
	.fetch_all(&state.pool)
	.await?;
    let now = chrono::Utc::now().timestamp_millis();
    Ok(ProjectionSnapshot {
        core_id: state.core_id.clone(),
        owner_user_id: config.owner_user_id.clone(),
        link_state: "linked",
        connection_url: Some(state.config.public_url.clone()),
        setup_mode,
        members: members
            .into_iter()
            .map(|member| ProjectionMember {
                is_owner: member.role == "owner"
                    || member.user_id.as_str() == config.owner_user_id.as_str(),
                user_id: member.user_id,
            })
            .collect(),
        revision: now,
        last_seen_at: now,
        synced_at: now,
    })
}
