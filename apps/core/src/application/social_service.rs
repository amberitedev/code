use std::sync::Arc;

use crate::application::social_lookup_service::{
    ensure_metadata, get_member, now, validate_choice,
};
use crate::application::social_models::{
    BanMemberRequest, CoreGroupBan, CoreMember, CoreMetadata, SocialError,
    SyncProfile, UpdateCoreMetadataRequest, UpsertMemberRequest,
};
use crate::application::state::AppState;

pub async fn get_core_metadata(
    state: &Arc<AppState>,
) -> Result<CoreMetadata, SocialError> {
    ensure_metadata(state).await?;
    let row: (String, Option<String>, Option<String>, Option<String>, String, String, String) = sqlx::query_as(
		"SELECT name, description, banner, subdomain, setup_mode, run_mode, updated_at FROM core_metadata WHERE id = 1",
	)
	.fetch_one(&state.pool)
	.await?;
    Ok(CoreMetadata {
        core_id: state.core_id.clone(),
        name: row.0,
        description: row.1,
        banner: row.2,
        subdomain: row.3,
        setup_mode: row.4,
        run_mode: row.5,
        updated_at: row.6,
    })
}

pub async fn update_core_metadata(
    state: &Arc<AppState>,
    req: UpdateCoreMetadataRequest,
) -> Result<CoreMetadata, SocialError> {
    ensure_metadata(state).await?;
    if let Some(mode) = &req.setup_mode {
        validate_choice(mode, &["remote", "local"], "setup_mode")?;
    }
    if let Some(mode) = &req.run_mode {
        validate_choice(mode, &["manual", "app_open", "startup"], "run_mode")?;
    }
    sqlx::query("UPDATE core_metadata SET name = COALESCE(?, name), description = COALESCE(?, description), banner = COALESCE(?, banner), subdomain = COALESCE(?, subdomain), setup_mode = COALESCE(?, setup_mode), run_mode = COALESCE(?, run_mode), updated_at = ? WHERE id = 1")
		.bind(req.name).bind(req.description).bind(req.banner).bind(req.subdomain)
		.bind(req.setup_mode).bind(req.run_mode).bind(now()).execute(&state.pool).await?;
    get_core_metadata(state).await
}

pub async fn list_members(
    state: &Arc<AppState>,
) -> Result<Vec<CoreMember>, SocialError> {
    Ok(sqlx::query_as(
        "SELECT * FROM core_members ORDER BY role, display_name, user_id",
    )
    .fetch_all(&state.pool)
    .await?)
}

pub async fn upsert_member(
    state: &Arc<AppState>,
    req: UpsertMemberRequest,
) -> Result<CoreMember, SocialError> {
    validate_choice(&req.role, &["owner", "admin", "member"], "role")?;
    let owner_user_id = state.owner_user_id().await;
    if req.role == "owner"
        && owner_user_id.as_deref() != Some(req.user_id.as_str())
    {
        return Err(SocialError::Invalid(
            "owner role cannot be assigned".into(),
        ));
    }
    if req.role != "owner"
        && owner_user_id.as_deref() == Some(req.user_id.as_str())
    {
        return Err(SocialError::Invalid(
            "owner role cannot be changed".into(),
        ));
    }
    let preset = req.permission_preset.unwrap_or_else(|| req.role.clone());
    let custom = match req.custom_permissions {
        Some(value) => Some(
            serde_json::to_string(&value)
                .map_err(|e| SocialError::Invalid(e.to_string()))?,
        ),
        None => None,
    };
    let now = now();
    sqlx::query("INSERT INTO core_members (user_id, display_name, role, permission_preset, custom_permissions, status, joined_at, updated_at) VALUES (?, ?, ?, ?, ?, 'active', ?, ?) ON CONFLICT(user_id) DO UPDATE SET display_name = excluded.display_name, role = excluded.role, permission_preset = excluded.permission_preset, custom_permissions = excluded.custom_permissions, status = 'active', updated_at = excluded.updated_at")
		.bind(&req.user_id).bind(req.display_name).bind(req.role).bind(preset).bind(custom).bind(&now).bind(&now).execute(&state.pool).await?;
    get_member(state, &req.user_id).await
}

pub async fn remove_member(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<(), SocialError> {
    let deleted = sqlx::query(
        "DELETE FROM core_members WHERE user_id = ? AND role != 'owner'",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?
    .rows_affected();
    if deleted == 0 {
        let role: Option<String> = sqlx::query_scalar(
            "SELECT role FROM core_members WHERE user_id = ?",
        )
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?;
        if role.as_deref() == Some("owner") {
            return Err(SocialError::Invalid("owner cannot be removed".into()));
        }
    }
    Ok(())
}

pub async fn ban_member(
    state: &Arc<AppState>,
    req: BanMemberRequest,
    banned_by: &str,
) -> Result<CoreGroupBan, SocialError> {
    let role: Option<String> =
        sqlx::query_scalar("SELECT role FROM core_members WHERE user_id = ?")
            .bind(&req.user_id)
            .fetch_optional(&state.pool)
            .await?;
    if role.as_deref() == Some("owner") {
        return Err(SocialError::Invalid("owner cannot be banned".into()));
    }
    let now = now();
    sqlx::query("INSERT OR REPLACE INTO core_group_bans (user_id, reason, banned_by, banned_at) VALUES (?, ?, ?, ?)")
		.bind(&req.user_id).bind(req.reason).bind(banned_by).bind(&now).execute(&state.pool).await?;
    remove_member(state, &req.user_id).await?;
    sqlx::query("DELETE FROM instance_members WHERE user_id = ?")
        .bind(&req.user_id)
        .execute(&state.pool)
        .await?;
    Ok(
        sqlx::query_as("SELECT * FROM core_group_bans WHERE user_id = ?")
            .bind(&req.user_id)
            .fetch_one(&state.pool)
            .await?,
    )
}

pub async fn list_bans(
    state: &Arc<AppState>,
) -> Result<Vec<CoreGroupBan>, SocialError> {
    Ok(
        sqlx::query_as("SELECT * FROM core_group_bans ORDER BY banned_at DESC")
            .fetch_all(&state.pool)
            .await?,
    )
}

pub async fn list_sync_profiles(
    state: &Arc<AppState>,
) -> Result<Vec<SyncProfile>, SocialError> {
    Ok(
        sqlx::query_as("SELECT * FROM sync_profiles ORDER BY updated_at DESC")
            .fetch_all(&state.pool)
            .await?,
    )
}

pub async fn remove_sync_profile(
    state: &Arc<AppState>,
    id: &str,
) -> Result<(), SocialError> {
    let deleted = sqlx::query("DELETE FROM sync_profiles WHERE id = ?")
        .bind(id)
        .execute(&state.pool)
        .await?
        .rows_affected();
    if deleted == 0 {
        return Err(SocialError::NotFound);
    }
    Ok(())
}
