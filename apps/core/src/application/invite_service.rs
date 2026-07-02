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
    if active_member_exists(state, &request.invitee_user_id).await? {
        return Err(SocialError::Invalid(
            "user already has Core access".into(),
        ));
    }

    let invitee_display_name = request.invitee_display_name.clone();
    let timestamp = now();
    let status = if approval == 0 || can_approve {
        "sent"
    } else {
        "pending_review"
    };
    let role_snapshot = serde_json::to_string(&role)
        .map_err(|error| SocialError::Invalid(error.to_string()))?;
    if let Some(existing_id) =
        active_invitation_id(state, &request.invitee_user_id).await?
    {
        sqlx::query("UPDATE core_invitations SET invitee_display_name = ?, role_id = ?, role_snapshot_json = ?, inviter_user_id = ?, status = ?, updated_at = ?, expires_at = datetime('now', '+7 days'), reviewed_by_user_id = NULL WHERE id = ?")
			.bind(invitee_display_name)
			.bind(&role.id)
			.bind(&role_snapshot)
			.bind(actor)
			.bind(status)
			.bind(&timestamp)
			.bind(&existing_id)
			.execute(&state.pool)
			.await?;
        return get(state, &existing_id).await;
    }

    let id = Uuid::new_v4().to_string();
    sqlx::query("INSERT INTO core_invitations (id, invitee_user_id, invitee_display_name, role_id, role_snapshot_json, inviter_user_id, status, created_at, updated_at, expires_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, datetime('now', '+7 days'))")
		.bind(&id).bind(&request.invitee_user_id).bind(invitee_display_name).bind(&role.id).bind(role_snapshot).bind(actor).bind(status).bind(&timestamp).bind(&timestamp).execute(&state.pool).await?;
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

pub async fn revoke(
    state: &Arc<AppState>,
    actor: &str,
    id: &str,
) -> Result<CoreInvitation, SocialError> {
    let viewer = access_service::require_core_member(state, actor).await?;
    if !viewer.permissions.iter().any(|permission| {
        permission == "invite-members"
            || permission == "approve-invites"
            || permission == "members:manage"
    }) {
        return Err(SocialError::Invalid(
            "not authorized to revoke invitations".into(),
        ));
    }
    sqlx::query("UPDATE core_invitations SET status = 'revoked', reviewed_by_user_id = ?, updated_at = ? WHERE id = ? AND status IN ('pending_review', 'sent')")
		.bind(actor)
		.bind(now())
		.bind(id)
		.execute(&state.pool)
		.await?;
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
        let (role, permission_preset) = invitation_access(&invite)?;
        sqlx::query("INSERT INTO core_members (user_id, display_name, role, permission_preset, role_id, status, joined_at, updated_at) VALUES (?, ?, ?, ?, ?, 'active', ?, ?) ON CONFLICT(user_id) DO UPDATE SET display_name = excluded.display_name, role = excluded.role, permission_preset = excluded.permission_preset, role_id = excluded.role_id, role_snapshot_json = NULL, needs_role_reassignment_at = NULL, status = 'active', updated_at = excluded.updated_at")
			.bind(user_id).bind(&invite.invitee_display_name).bind(role).bind(permission_preset).bind(&invite.role_id).bind(&timestamp).bind(&timestamp).execute(&state.pool).await?;
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

async fn active_member_exists(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<bool, SocialError> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM core_members WHERE user_id = ? AND status = 'active'",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?
        > 0)
}

async fn active_invitation_id(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<Option<String>, SocialError> {
    Ok(sqlx::query_scalar(
        "SELECT id FROM core_invitations WHERE invitee_user_id = ? AND status IN ('pending_review', 'sent') AND expires_at > CURRENT_TIMESTAMP ORDER BY created_at DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?)
}

fn invitation_access(
    invite: &CoreInvitation,
) -> Result<(&'static str, &'static str), SocialError> {
    if invite.role_id == "role-admin" {
        return Ok(("admin", "admin"));
    }
    let role = serde_json::from_str::<role_service::CoreRole>(
        &invite.role_snapshot_json,
    )
    .map_err(|error| SocialError::Invalid(error.to_string()))?;
    let grants = serde_json::from_str::<Vec<String>>(&role.grants_json)
        .unwrap_or_default();
    if grants.iter().any(|grant| {
        matches!(
            grant.as_str(),
            "invite-members"
                | "remove-members"
                | "ban-members"
                | "manage-roles"
                | "edit-member-roles"
                | "approve-invites"
                | "manage-instances"
                | "edit-settings"
        )
    }) {
        Ok(("admin", "admin"))
    } else {
        Ok(("member", "member"))
    }
}
