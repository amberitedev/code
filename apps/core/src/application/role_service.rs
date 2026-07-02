use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::{
    social_lookup_service::now, social_models::SocialError, state::AppState,
};

pub const MAX_ACTIVE_ROLES: i64 = 4;

#[derive(Debug, Deserialize, Serialize, sqlx::FromRow, Clone)]
pub struct CoreRole {
    pub id: String,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub grants_json: String,
    pub retired_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Deserialize)]
pub struct SaveCoreRoleRequest {
    pub id: Option<String>,
    pub name: String,
    pub description: String,
    pub icon: String,
    pub grants: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct RoleSettingsRequest {
    pub require_invite_approval: bool,
}

#[derive(Debug, Serialize)]
pub struct RoleConfiguration {
    pub roles: Vec<CoreRole>,
    pub require_invite_approval: bool,
}

pub async fn list(
    state: &Arc<AppState>,
) -> Result<RoleConfiguration, SocialError> {
    let roles = sqlx::query_as(
        "SELECT * FROM core_roles WHERE retired_at IS NULL ORDER BY created_at",
    )
    .fetch_all(&state.pool)
    .await?;
    let approval = sqlx::query_scalar::<_, i64>(
        "SELECT require_invite_approval FROM core_role_settings WHERE id = 1",
    )
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(1)
        != 0;
    Ok(RoleConfiguration {
        roles,
        require_invite_approval: approval,
    })
}

pub async fn save(
    state: &Arc<AppState>,
    request: SaveCoreRoleRequest,
) -> Result<CoreRole, SocialError> {
    let name = request.name.trim();
    if name.is_empty()
        || name.chars().count() > 20
        || request.icon.trim().is_empty()
    {
        return Err(SocialError::Invalid("invalid role details".into()));
    }
    let grants = serde_json::to_string(&request.grants)
        .map_err(|error| SocialError::Invalid(error.to_string()))?;
    let id = request.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    if !exists(state, &id).await? {
        let count = active_count(state).await?;
        if count >= MAX_ACTIVE_ROLES {
            return Err(SocialError::Invalid(
                "a Core may have at most four roles".into(),
            ));
        }
        let timestamp = now();
        sqlx::query("INSERT INTO core_roles (id, name, description, icon, grants_json, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
			.bind(&id).bind(name).bind(request.description.trim()).bind(request.icon.trim()).bind(&grants).bind(&timestamp).bind(&timestamp)
			.execute(&state.pool).await?;
    } else {
        sqlx::query("UPDATE core_roles SET name = ?, description = ?, icon = ?, grants_json = ?, updated_at = ? WHERE id = ? AND retired_at IS NULL")
			.bind(name).bind(request.description.trim()).bind(request.icon.trim()).bind(&grants).bind(now()).bind(&id)
			.execute(&state.pool).await?;
    }
    get(state, &id).await
}

pub async fn retire(
    state: &Arc<AppState>,
    id: &str,
) -> Result<(), SocialError> {
    if active_count(state).await? <= 1 {
        return Err(SocialError::Invalid(
            "a Core needs at least one active role".into(),
        ));
    }
    let role = get(state, id).await?;
    if role.retired_at.is_some() {
        return Ok(());
    }
    let snapshot = serde_json::to_string(&role)
        .map_err(|error| SocialError::Invalid(error.to_string()))?;
    let timestamp = now();
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "UPDATE core_roles SET retired_at = ?, updated_at = ? WHERE id = ?",
    )
    .bind(&timestamp)
    .bind(&timestamp)
    .bind(id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("UPDATE core_members SET role_snapshot_json = ?, needs_role_reassignment_at = ? WHERE role_id = ?").bind(&snapshot).bind(&timestamp).bind(id).execute(&mut *tx).await?;
    sqlx::query("UPDATE core_invitations SET role_snapshot_json = ? WHERE role_id = ? AND status IN ('pending_review', 'sent')").bind(&snapshot).bind(id).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn update_settings(
    state: &Arc<AppState>,
    request: RoleSettingsRequest,
) -> Result<(), SocialError> {
    sqlx::query("UPDATE core_role_settings SET require_invite_approval = ?, updated_at = ? WHERE id = 1")
		.bind(i64::from(request.require_invite_approval)).bind(now()).execute(&state.pool).await?;
    Ok(())
}

pub async fn get(
    state: &Arc<AppState>,
    id: &str,
) -> Result<CoreRole, SocialError> {
    sqlx::query_as("SELECT * FROM core_roles WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(SocialError::NotFound)
}

async fn exists(state: &Arc<AppState>, id: &str) -> Result<bool, SocialError> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM core_roles WHERE id = ?",
    )
    .bind(id)
    .fetch_one(&state.pool)
    .await?
        > 0)
}

async fn active_count(state: &Arc<AppState>) -> Result<i64, SocialError> {
    Ok(sqlx::query_scalar(
        "SELECT COUNT(*) FROM core_roles WHERE retired_at IS NULL",
    )
    .fetch_one(&state.pool)
    .await?)
}
