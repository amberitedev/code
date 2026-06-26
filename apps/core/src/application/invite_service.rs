use std::sync::Arc;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::application::{
    access_service, role_service, social_lookup_service::now,
    social_models::SocialError, state::AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct CoreInvitation {
    pub id: String,
    pub invitee_user_id: String,
    pub invitee_display_name: Option<String>,
    pub role_id: String,
    pub role_snapshot_json: String,
    pub inviter_user_id: String,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
    pub expires_at: String,
    pub responded_at: Option<String>,
    pub reviewed_by_user_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateInvitationRequest {
    pub invitee_user_id: String,
    pub invitee_display_name: Option<String>,
    pub role_id: String,
}

pub async fn create(
    state: &Arc<AppState>,
    actor: &str,
    request: CreateInvitationRequest,
) -> Result<CoreInvitation, SocialError> {
    let viewer = access_service::require_core_member(state, actor).await?;
    if !viewer.permissions.iter().any(|permission| {
        permission == "invite-members"
            || permission == "approve-invites"
            || permission == "members:manage"
    }) {
        return Err(SocialError::Invalid(
            "not authorized to invite members".into(),
        ));
    }
    let role = role_service::get(state, &request.role_id).await?;
    if role.retired_at.is_some() {
        return Err(SocialError::Invalid("role is retired".into()));
    }
    let approval: i64 = sqlx::query_scalar(
        "SELECT require_invite_approval FROM core_role_settings WHERE id = 1",
    )
    .fetch_optional(&state.pool)
    .await?
    .unwrap_or(1);
    let can_approve = viewer.permissions.iter().any(|permission| {
        permission == "approve-invites" || permission == "members:manage"
    });
    let timestamp = now();
    let id = Uuid::new_v4().to_string();
    let status = if approval == 0 || can_approve {
        "sent"
    } else {
        "pending_review"
    };
    sqlx::query("INSERT INTO core_invitations (id, invitee_user_id, invitee_display_name, role_id, role_snapshot_json, inviter_user_id, status, created_at, updated_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now', '+7 days'))")
		.bind(&id).bind(&request.invitee_user_id).bind(request.invitee_display_name).bind(&role.id).bind(serde_json::to_string(&role).map_err(|error| SocialError::Invalid(error.to_string()))?).bind(actor).bind(status).bind(&timestamp).bind(&timestamp).execute(&state.pool).await?;
    get(state, &id).await
}

pub async fn list(
    state: &Arc<AppState>,
) -> Result<Vec<CoreInvitation>, SocialError> {
    Ok(sqlx::query_as(
        "SELECT * FROM core_invitations ORDER BY created_at DESC",
    )
    .fetch_all(&state.pool)
    .await?)
}

pub async fn list_mine(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<Vec<CoreInvitation>, SocialError> {
    Ok(sqlx::query_as("SELECT * FROM core_invitations WHERE invitee_user_id = ? AND status = 'sent' AND expires_at > CURRENT_TIMESTAMP ORDER BY created_at DESC").bind(user_id).fetch_all(&state.pool).await?)
}

pub async fn review(
    state: &Arc<AppState>,
    actor: &str,
    id: &str,
    accept: bool,
) -> Result<CoreInvitation, SocialError> {
    let viewer = access_service::require_core_member(state, actor).await?;
    if !viewer.permissions.iter().any(|permission| {
        permission == "approve-invites" || permission == "members:manage"
    }) {
        return Err(SocialError::Invalid(
            "not authorized to review invitations".into(),
        ));
    }
    sqlx::query("UPDATE core_invitations SET status = ?, reviewed_by_user_id = ?, updated_at = ? WHERE id = ? AND status = 'pending_review'").bind(if accept { "sent" } else { "rejected" }).bind(actor).bind(now()).bind(id).execute(&state.pool).await?;
    get(state, id).await
}

pub async fn respond(
    state: &Arc<AppState>,
    user_id: &str,
    id: &str,
    accept: bool,
) -> Result<CoreInvitation, SocialError> {
    let invite = get(state, id).await?;
    if invite.invitee_user_id != user_id || invite.status != "sent" {
        return Err(SocialError::Invalid(
            "invite cannot be responded to".into(),
        ));
    }
    let expired: i64 = sqlx::query_scalar(
		"SELECT CASE WHEN expires_at <= CURRENT_TIMESTAMP THEN 1 ELSE 0 END FROM core_invitations WHERE id = ?",
	)
	.bind(id)
	.fetch_one(&state.pool)
	.await?;
    if expired != 0 {
        return Err(SocialError::Invalid("invite has expired".into()));
    }
    let timestamp = now();
    if accept {
        sqlx::query("INSERT INTO core_members (user_id, display_name, role, permission_preset, role_id, status, joined_at, updated_at) VALUES (?, ?, 'member', 'member', ?, 'active', ?, ?) ON CONFLICT(user_id) DO UPDATE SET display_name = excluded.display_name, role_id = excluded.role_id, role_snapshot_json = NULL, needs_role_reassignment_at = NULL, status = 'active', updated_at = excluded.updated_at")
			.bind(user_id).bind(&invite.invitee_display_name).bind(&invite.role_id).bind(&timestamp).bind(&timestamp).execute(&state.pool).await?;
    }
    sqlx::query("UPDATE core_invitations SET status = ?, responded_at = ?, updated_at = ? WHERE id = ?").bind(if accept { "accepted" } else { "declined" }).bind(&timestamp).bind(&timestamp).bind(id).execute(&state.pool).await?;
    get(state, id).await
}

async fn get(
    state: &Arc<AppState>,
    id: &str,
) -> Result<CoreInvitation, SocialError> {
    sqlx::query_as("SELECT * FROM core_invitations WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(SocialError::NotFound)
}
