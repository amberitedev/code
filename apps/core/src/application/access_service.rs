//! Core and instance access workflows.
//!
//! Key entry points: list access responses (lines 18 and 35), mutate Core and
//! instance grants (lines 67 and 137), enforce route permissions (line 237),
//! calculate effective access with bans/custom grants (line 267), and map role
//! presets/grants into API-facing permissions (line 367).

use std::sync::Arc;

use serde_json::Value;

use crate::application::{
    social_lookup_service::now,
    social_models::{
        AccessResponse, CoreMember, EffectiveAccessMember,
        EffectiveAccessViewer, InstanceMember, PatchAccessRequest, SocialError,
        UpsertAccessRequest,
    },
    state::AppState,
};

const ROLES: &[&str] = &["owner", "admin", "member"];
const PRESETS: &[&str] = &["owner", "admin", "member", "viewer", "client-only"];

pub async fn list_core_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
) -> Result<AccessResponse, SocialError> {
    let members: Vec<CoreMember> = sqlx::query_as(
        "SELECT * FROM core_members ORDER BY role, display_name, user_id",
    )
    .fetch_all(&state.pool)
    .await?;
    let viewer = effective_viewer(state, actor_user_id, None).await?;
    Ok(AccessResponse {
        members: members.into_iter().map(core_member_to_effective).collect(),
        viewer,
    })
}

pub async fn list_instance_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
    instance_id: &str,
) -> Result<AccessResponse, SocialError> {
    let core_members: Vec<CoreMember> = sqlx::query_as(
        "SELECT * FROM core_members ORDER BY role, display_name, user_id",
    )
    .fetch_all(&state.pool)
    .await?;
    let instance_members: Vec<InstanceMember> = sqlx::query_as(
        "SELECT * FROM instance_members WHERE instance_id = ? ORDER BY role, display_name, user_id",
    )
    .bind(instance_id)
    .fetch_all(&state.pool)
    .await?;

    let mut members: Vec<EffectiveAccessMember> = core_members
        .into_iter()
        .map(core_member_to_effective)
        .collect();
    for member in instance_members {
        if let Some(existing) =
            members.iter_mut().find(|row| row.user_id == member.user_id)
        {
            *existing = instance_member_to_effective(member);
        } else {
            members.push(instance_member_to_effective(member));
        }
    }
    let viewer =
        effective_viewer(state, actor_user_id, Some(instance_id)).await?;
    Ok(AccessResponse { members, viewer })
}

pub async fn upsert_core_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
    req: UpsertAccessRequest,
) -> Result<CoreMember, SocialError> {
    require_core_manager(state, actor_user_id).await?;
    validate_role(&req.role)?;
    let preset = validate_preset(req.permission_preset.as_deref(), &req.role)?;
    guard_owner_change(state, &req.user_id, &req.role).await?;
    let custom = serialize_custom(req.custom_permissions)?;
    let now = now();
    sqlx::query("INSERT INTO core_members (user_id, display_name, role, permission_preset, custom_permissions, status, joined_at, updated_at) VALUES (?, ?, ?, ?, ?, 'active', ?, ?) ON CONFLICT(user_id) DO UPDATE SET display_name = excluded.display_name, role = excluded.role, permission_preset = excluded.permission_preset, custom_permissions = excluded.custom_permissions, status = 'active', updated_at = excluded.updated_at")
		.bind(&req.user_id).bind(req.display_name).bind(&req.role).bind(preset).bind(custom)
		.bind(&now).bind(&now).execute(&state.pool).await?;
    get_core_member(state, &req.user_id).await
}

pub async fn patch_core_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
    user_id: &str,
    req: PatchAccessRequest,
) -> Result<CoreMember, SocialError> {
    require_core_manager(state, actor_user_id).await?;
    let current = get_core_member(state, user_id).await?;
    let role = req.role.unwrap_or_else(|| current.role.clone());
    validate_role(&role)?;
    guard_owner_change(state, user_id, &role).await?;
    let preset = match req.permission_preset {
        Some(value) => validate_preset(Some(&value), &role)?.to_string(),
        None => current.permission_preset.clone(),
    };
    let custom = match req.custom_permissions {
        Some(value) => Some(serde_json::to_string(&value).map_err(invalid)?),
        None => current.custom_permissions,
    };
    sqlx::query("UPDATE core_members SET display_name = COALESCE(?, display_name), role = ?, permission_preset = ?, custom_permissions = ?, status = 'active', updated_at = ? WHERE user_id = ?")
		.bind(req.display_name).bind(&role).bind(&preset).bind(custom).bind(now()).bind(user_id)
		.execute(&state.pool).await?;
    get_core_member(state, user_id).await
}

pub async fn remove_core_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
    user_id: &str,
) -> Result<(), SocialError> {
    require_core_manager(state, actor_user_id).await?;
    if state.owner_user_id().await.as_deref() == Some(user_id) {
        return Err(SocialError::Invalid("owner cannot be removed".into()));
    }
    sqlx::query(
        "DELETE FROM core_members WHERE user_id = ? AND role != 'owner'",
    )
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn upsert_instance_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
    instance_id: &str,
    req: UpsertAccessRequest,
) -> Result<InstanceMember, SocialError> {
    require_instance_manager(state, actor_user_id, instance_id).await?;
    validate_role(&req.role)?;
    let preset = validate_preset(req.permission_preset.as_deref(), &req.role)?;
    let custom = serialize_custom(req.custom_permissions)?;
    let now = now();
    sqlx::query("INSERT INTO instance_members (instance_id, user_id, display_name, role, permission_preset, custom_permissions, status, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, 'active', ?, ?) ON CONFLICT(instance_id, user_id) DO UPDATE SET display_name = excluded.display_name, role = excluded.role, permission_preset = excluded.permission_preset, custom_permissions = excluded.custom_permissions, status = 'active', updated_at = excluded.updated_at")
		.bind(instance_id).bind(&req.user_id).bind(req.display_name).bind(&req.role).bind(preset)
		.bind(custom).bind(&now).bind(&now).execute(&state.pool).await?;
    get_instance_member(state, instance_id, &req.user_id).await
}

pub async fn patch_instance_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
    instance_id: &str,
    user_id: &str,
    req: PatchAccessRequest,
) -> Result<InstanceMember, SocialError> {
    require_instance_manager(state, actor_user_id, instance_id).await?;
    let current = get_instance_member(state, instance_id, user_id).await?;
    let role = req.role.unwrap_or_else(|| current.role.clone());
    validate_role(&role)?;
    let preset = match req.permission_preset {
        Some(value) => validate_preset(Some(&value), &role)?.to_string(),
        None => current.permission_preset.clone(),
    };
    let custom = match req.custom_permissions {
        Some(value) => Some(serde_json::to_string(&value).map_err(invalid)?),
        None => current.custom_permissions,
    };
    sqlx::query("UPDATE instance_members SET display_name = COALESCE(?, display_name), role = ?, permission_preset = ?, custom_permissions = ?, status = 'active', updated_at = ? WHERE instance_id = ? AND user_id = ?")
		.bind(req.display_name).bind(&role).bind(&preset).bind(custom).bind(now())
		.bind(instance_id).bind(user_id).execute(&state.pool).await?;
    get_instance_member(state, instance_id, user_id).await
}

pub async fn remove_instance_access(
    state: &Arc<AppState>,
    actor_user_id: &str,
    instance_id: &str,
    user_id: &str,
) -> Result<(), SocialError> {
    require_instance_manager(state, actor_user_id, instance_id).await?;
    sqlx::query(
        "DELETE FROM instance_members WHERE instance_id = ? AND user_id = ?",
    )
    .bind(instance_id)
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    Ok(())
}

pub async fn require_core_member(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<EffectiveAccessViewer, SocialError> {
    effective_viewer(state, user_id, None).await
}

pub async fn require_any_member(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<EffectiveAccessViewer, SocialError> {
    reject_banned(state, user_id).await?;
    match effective_viewer(state, user_id, None).await {
        Ok(viewer) => Ok(viewer),
        Err(core_error) => {
            let member: Option<InstanceMember> = sqlx::query_as(
                "SELECT * FROM instance_members WHERE user_id = ? AND status = 'active' LIMIT 1",
            )
            .bind(user_id)
            .fetch_optional(&state.pool)
            .await?;
            member
                .map(|member| {
                    viewer_with_custom(
                        user_id,
                        &member.role,
                        &member.permission_preset,
                        member.custom_permissions.as_deref(),
                    )
                })
                .ok_or(core_error)
        }
    }
}

pub async fn require_core_manager(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<EffectiveAccessViewer, SocialError> {
    let viewer = effective_viewer(state, user_id, None).await?;
    if viewer.can_manage_users {
        Ok(viewer)
    } else {
        Err(SocialError::Invalid(
            "not authorized to manage access".into(),
        ))
    }
}

pub async fn require_instance_manager(
    state: &Arc<AppState>,
    user_id: &str,
    instance_id: &str,
) -> Result<EffectiveAccessViewer, SocialError> {
    let viewer = effective_viewer(state, user_id, Some(instance_id)).await?;
    if viewer.can_manage_users {
        Ok(viewer)
    } else {
        Err(SocialError::Invalid(
            "not authorized to manage access".into(),
        ))
    }
}

pub async fn require_instance_permission(
    state: &Arc<AppState>,
    user_id: &str,
    instance_id: &str,
    permission: &str,
) -> Result<EffectiveAccessViewer, SocialError> {
    let viewer = effective_viewer(state, user_id, Some(instance_id)).await?;
    if viewer.permissions.iter().any(|value| value == permission) {
        Ok(viewer)
    } else {
        Err(SocialError::Invalid(format!(
            "not authorized for {permission}"
        )))
    }
}

async fn effective_viewer(
    state: &Arc<AppState>,
    user_id: &str,
    instance_id: Option<&str>,
) -> Result<EffectiveAccessViewer, SocialError> {
    reject_banned(state, user_id).await?;
    if state.config.dev_mode && user_id == "dev-owner" {
        return Ok(viewer(user_id, "owner", "owner"));
    }
    if state.owner_user_id().await.as_deref() == Some(user_id) {
        return Ok(viewer(user_id, "owner", "owner"));
    }
    if let Some(instance_id) = instance_id {
        if let Some(member) = sqlx::query_as::<_, InstanceMember>(
            "SELECT * FROM instance_members WHERE instance_id = ? AND user_id = ? AND status = 'active'",
        )
        .bind(instance_id)
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        {
			return Ok(viewer_with_custom(
				user_id,
				&member.role,
				&member.permission_preset,
				member.custom_permissions.as_deref(),
			));
		}
    }
    let member: Option<CoreMember> = sqlx::query_as(
        "SELECT * FROM core_members WHERE user_id = ? AND status = 'active'",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    let member = member
        .ok_or(SocialError::Invalid("not authorized for this Core".into()))?;
    viewer_for_core_member(state, user_id, member).await
}

async fn reject_banned(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<(), SocialError> {
    let banned: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM core_group_bans WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await?;
    if banned > 0 {
        return Err(SocialError::Invalid(
            "not authorized for this Core".into(),
        ));
    }
    Ok(())
}

async fn viewer_for_core_member(
    state: &Arc<AppState>,
    user_id: &str,
    member: CoreMember,
) -> Result<EffectiveAccessViewer, SocialError> {
    let permissions =
        if let Some(snapshot) = member.role_snapshot_json.as_deref() {
            serde_json::from_str::<serde_json::Value>(snapshot)
                .ok()
                .and_then(|value| {
                    value
                        .get("grants_json")
                        .and_then(|grants| grants.as_str())
                        .map(str::to_string)
                })
                .and_then(|grants| {
                    serde_json::from_str::<Vec<String>>(&grants).ok()
                })
                .unwrap_or_else(|| permissions_for(&member.permission_preset))
        } else if let Some(role_id) = member.role_id.as_deref() {
            let grants: Option<String> = sqlx::query_scalar(
                "SELECT grants_json FROM core_roles WHERE id = ?",
            )
            .bind(role_id)
            .fetch_optional(&state.pool)
            .await?;
            grants
                .and_then(|value| serde_json::from_str(&value).ok())
                .unwrap_or_else(|| permissions_for(&member.permission_preset))
        } else {
            permissions_for(&member.permission_preset)
        };
    let permissions = expand_role_grants(apply_custom_permissions(
        permissions,
        member.custom_permissions.as_deref(),
    ));
    let permissions = apply_custom_permissions(
        permissions,
        member.custom_permissions.as_deref(),
    );
    Ok(EffectiveAccessViewer {
        user_id: user_id.to_string(),
        role: member.role,
        permission_preset: member.permission_preset,
        can_manage_users: can_manage_users(&permissions),
        permissions,
    })
}

fn expand_role_grants(grants: Vec<String>) -> Vec<String> {
    let mut permissions = grants.clone();
    if grants.iter().any(|grant| {
        matches!(
            grant.as_str(),
            "manage-instances"
                | "start-stop-instances"
                | "restart-instances"
                | "read-console"
                | "write-console"
                | "manage-mods"
                | "manage-worlds"
                | "manage-files"
                | "manage-backups"
                | "manage-network"
        )
    }) {
        permissions.push("server:view".to_string());
    }
    for grant in grants {
        match grant.as_str() {
            "manage-instances"
            | "start-stop-instances"
            | "restart-instances" => {
                permissions.push("server:power".to_string())
            }
            "manage-mods" | "manage-worlds" => {
                permissions.push("server:content".to_string())
            }
            "manage-files" => permissions.push("server:files".to_string()),
            "manage-backups" => permissions.push("server:backups".to_string()),
            "manage-network" | "edit-settings" => {
                permissions.push("server:settings".to_string())
            }
            "read-console" | "write-console" => {
                permissions.push("server:console".to_string())
            }
            "edit-member-roles" | "manage-roles" => {
                permissions.push("members:manage".to_string())
            }
            _ => {}
        }
    }
    permissions.sort();
    permissions.dedup();
    permissions
}

fn viewer(user_id: &str, role: &str, preset: &str) -> EffectiveAccessViewer {
    viewer_with_custom(user_id, role, preset, None)
}

fn viewer_with_custom(
    user_id: &str,
    role: &str,
    preset: &str,
    custom_permissions: Option<&str>,
) -> EffectiveAccessViewer {
    let permissions = expand_role_grants(apply_custom_permissions(
        permissions_for(preset),
        custom_permissions,
    ));
    let permissions = apply_custom_permissions(permissions, custom_permissions);
    EffectiveAccessViewer {
        user_id: user_id.to_string(),
        role: role.to_string(),
        permission_preset: preset.to_string(),
        can_manage_users: can_manage_users(&permissions),
        permissions,
    }
}

fn can_manage_users(permissions: &[String]) -> bool {
    permissions.iter().any(|value| {
        matches!(
            value.as_str(),
            "members:manage" | "manage-roles" | "edit-member-roles"
        )
    })
}

fn apply_custom_permissions(
    mut permissions: Vec<String>,
    custom_permissions: Option<&str>,
) -> Vec<String> {
    let Some(custom_permissions) = custom_permissions else {
        return permissions;
    };
    let Ok(value) = serde_json::from_str::<Value>(custom_permissions) else {
        return permissions;
    };
    let (additions, removals) = custom_permission_delta(&value);
    permissions
        .retain(|permission| !removals.iter().any(|value| value == permission));
    for permission in additions {
        if !permissions.iter().any(|value| value == &permission) {
            permissions.push(permission);
        }
    }
    permissions.sort();
    permissions.dedup();
    permissions
}

fn custom_permission_delta(value: &Value) -> (Vec<String>, Vec<String>) {
    match value {
        Value::Array(values) => (string_values(values), vec![]),
        Value::Object(map) => {
            let mut additions = Vec::new();
            let mut removals = Vec::new();
            for (key, value) in map {
                match (key.as_str(), value) {
                    (
                        "grants" | "allow" | "permissions",
                        Value::Array(values),
                    ) => {
                        additions.extend(string_values(values));
                    }
                    ("denies" | "remove", Value::Array(values)) => {
                        removals.extend(string_values(values));
                    }
                    (_, Value::Bool(true)) => additions.push(key.clone()),
                    (_, Value::Bool(false)) => removals.push(key.clone()),
                    _ => {}
                }
            }
            (additions, removals)
        }
        _ => (vec![], vec![]),
    }
}

fn string_values(values: &[Value]) -> Vec<String> {
    values
        .iter()
        .filter_map(|value| value.as_str())
        .map(str::to_string)
        .collect()
}

fn permissions_for(preset: &str) -> Vec<String> {
    let values = match preset {
        "owner" | "admin" => vec![
            "server:view",
            "server:power",
            "server:content",
            "server:files",
            "server:backups",
            "server:settings",
            "server:players",
            "server:console",
            "server:logs",
            "client:view",
            "client:content",
            "client:settings",
            "instance:settings",
            "members:manage",
        ],
        "member" => vec![
            "server:view",
            "client:view",
            "client:content",
            "client:settings",
            "instance:settings",
        ],
        "client-only" => {
            vec!["client:view", "client:content", "client:settings"]
        }
        _ => vec!["server:view", "client:view"],
    };
    values.into_iter().map(str::to_string).collect()
}

fn core_member_to_effective(member: CoreMember) -> EffectiveAccessMember {
    EffectiveAccessMember {
        user_id: member.user_id,
        display_name: member.display_name,
        role: member.role,
        permission_preset: member.permission_preset,
        custom_permissions: member.custom_permissions,
        status: member.status,
        joined_at: member.joined_at,
        updated_at: member.updated_at,
        source: "core".to_string(),
        role_id: member.role_id,
        role_snapshot_json: member.role_snapshot_json,
        needs_role_reassignment_at: member.needs_role_reassignment_at,
    }
}

fn instance_member_to_effective(
    member: InstanceMember,
) -> EffectiveAccessMember {
    EffectiveAccessMember {
        user_id: member.user_id,
        display_name: member.display_name,
        role: member.role,
        permission_preset: member.permission_preset,
        custom_permissions: member.custom_permissions,
        status: member.status,
        joined_at: member.created_at,
        updated_at: member.updated_at,
        source: "instance".to_string(),
        role_id: None,
        role_snapshot_json: None,
        needs_role_reassignment_at: None,
    }
}

async fn get_core_member(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<CoreMember, SocialError> {
    sqlx::query_as("SELECT * FROM core_members WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(SocialError::NotFound)
}

async fn get_instance_member(
    state: &Arc<AppState>,
    instance_id: &str,
    user_id: &str,
) -> Result<InstanceMember, SocialError> {
    sqlx::query_as(
        "SELECT * FROM instance_members WHERE instance_id = ? AND user_id = ?",
    )
    .bind(instance_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?
    .ok_or(SocialError::NotFound)
}

async fn guard_owner_change(
    state: &Arc<AppState>,
    user_id: &str,
    role: &str,
) -> Result<(), SocialError> {
    let owner = state.owner_user_id().await;
    if role == "owner" && owner.as_deref() != Some(user_id) {
        return Err(SocialError::Invalid(
            "owner role cannot be assigned".into(),
        ));
    }
    if role != "owner" && owner.as_deref() == Some(user_id) {
        return Err(SocialError::Invalid(
            "owner role cannot be changed".into(),
        ));
    }
    Ok(())
}

fn validate_role(role: &str) -> Result<(), SocialError> {
    if ROLES.contains(&role) {
        Ok(())
    } else {
        Err(SocialError::Invalid("invalid role".into()))
    }
}

fn validate_preset<'a>(
    preset: Option<&'a str>,
    fallback: &'a str,
) -> Result<&'a str, SocialError> {
    let value = preset.unwrap_or(fallback);
    if PRESETS.contains(&value) {
        Ok(value)
    } else {
        Err(SocialError::Invalid("invalid permission_preset".into()))
    }
}

fn serialize_custom(
    value: Option<Value>,
) -> Result<Option<String>, SocialError> {
    value
        .map(|value| serde_json::to_string(&value).map_err(invalid))
        .transpose()
}

fn invalid(error: impl std::fmt::Display) -> SocialError {
    SocialError::Invalid(error.to_string())
}
