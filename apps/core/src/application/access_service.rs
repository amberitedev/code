//! Server access: who may manage each server, with which role, and the activity log.
//!
//! The Core's owner is owner of every server. Anyone else is invited to one server at a time as
//! an editor or a viewer ("Limited" in the App), and has nothing until they accept. Roles and
//! their permission bits are Modrinth Hosting's, so the App's Access tab reads them unchanged.
//!
//! The backend only knows which accounts may reach this Core, so it can give them its address.
//! Core tells it when an account is invited and when it loses its last server here.

use std::sync::Arc;

use chrono::{SecondsFormat, Utc};
use reqwest::StatusCode;
use serde::Deserialize;
use serde_json::{json, Value};
use tracing::warn;

use crate::{
    application::state::AppState,
    presentation::{error::ApiError, extractors::Account},
};

/// Modrinth Hosting's permission bits (`UserScope`).
pub mod scope {
    pub const BASE_READ: u64 = 1 << 63;
    pub const POWER_ACTIONS: u64 = 1 << 62;
    pub const EXEC_COMMANDS: u64 = 1 << 61;
    pub const FILES_WRITE: u64 = 1 << 60;
    pub const SETUP: u64 = 1 << 59;
    pub const BACKUPS: u64 = 1 << 58;
    pub const ADVANCED: u64 = 1 << 57;
    pub const RESET_SERVER: u64 = 1 << 56;
    pub const MANAGE_USERS: u64 = 1 << 55;
    /// Every server permission. The low bits are Modrinth staff scopes, which nobody has here.
    pub const SERVER_ADMIN: u64 = !0 ^ ((1 << 15) - 1);
}

/// How long before an invite may be sent again.
const REINVITE_COOLDOWN_SECONDS: i64 = 2 * 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Role {
    Owner,
    Editor,
    Viewer,
}

impl Role {
    pub fn permissions(self) -> u64 {
        use scope::*;
        let viewer = BASE_READ | POWER_ACTIONS;
        match self {
            Self::Owner => SERVER_ADMIN,
            Self::Editor => {
                viewer
                    | EXEC_COMMANDS
                    | FILES_WRITE
                    | SETUP
                    | BACKUPS
                    | ADVANCED
            }
            Self::Viewer => viewer,
        }
    }

    /// The permissions as the signed number Modrinth's API sends.
    pub fn permissions_json(self) -> i64 {
        self.permissions() as i64
    }

    fn stored(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Editor => "editor",
            Self::Viewer => "viewer",
        }
    }

    /// A role that can be given to an invited account, as the App names it.
    pub fn assignable(name: &str) -> Result<Self, ApiError> {
        match name.to_ascii_lowercase().as_str() {
            "editor" => Ok(Self::Editor),
            "viewer" => Ok(Self::Viewer),
            _ => Err(ApiError::BadRequest(
                "role must be Editor or Viewer".into(),
            )),
        }
    }
}

fn now() -> String {
    // The same form as JavaScript's `toISOString`, so the App's time filters compare as text.
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn unavailable(error: reqwest::Error) -> ApiError {
    ApiError::ServiceUnavailable(format!(
        "Amberite could not be reached: {error}"
    ))
}

/// Remember an account's name and picture for the access list and the activity log.
pub async fn remember(state: &AppState, account: &Account) {
    let stored = sqlx::query(
        "INSERT INTO accounts (id, username, avatar_url) VALUES (?, ?, ?) \
         ON CONFLICT(id) DO UPDATE SET username = excluded.username, avatar_url = excluded.avatar_url",
    )
    .bind(&account.id)
    .bind(&account.username)
    .bind(&account.avatar_url)
    .execute(&state.pool)
    .await;
    if let Err(error) = stored {
        warn!(%error, "could not store an account's name");
    }
}

/// The role `user_id` has on a server. An invite that was not accepted gives none.
pub async fn role(
    state: &AppState,
    user_id: &str,
    server_id: &str,
) -> Result<Option<Role>, ApiError> {
    let Some(owner) = state.owner().await? else {
        return Err(ApiError::Forbidden("this Core is not paired".into()));
    };
    if owner.user_id == user_id {
        return Ok(Some(Role::Owner));
    }
    let stored: Option<String> = sqlx::query_scalar(
        "SELECT role FROM server_users WHERE server_id = ? AND user_id = ? AND added_on IS NOT NULL",
    )
    .bind(server_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    Ok(stored.map(|role| match role.as_str() {
        "editor" => Role::Editor,
        _ => Role::Viewer,
    }))
}

/// Refuse unless `user_id` has every permission in `scope` on the server.
pub async fn require(
    state: &AppState,
    user_id: &str,
    server_id: &str,
    scope: u64,
) -> Result<Role, ApiError> {
    let role = role(state, user_id, server_id).await?.ok_or_else(|| {
        ApiError::Forbidden("you do not have access to this server".into())
    })?;
    if role.permissions() & scope != scope {
        return Err(ApiError::Forbidden(
            "your role on this server does not allow this".into(),
        ));
    }
    Ok(role)
}

/// The servers `user_id` may see, or `None` for the owner, who sees all of them.
pub async fn visible_servers(
    state: &AppState,
    user_id: &str,
) -> Result<Option<Vec<String>>, ApiError> {
    let Some(owner) = state.owner().await? else {
        return Err(ApiError::Forbidden("this Core is not paired".into()));
    };
    if owner.user_id == user_id {
        return Ok(None);
    }
    Ok(Some(
        sqlx::query_scalar(
            "SELECT server_id FROM server_users WHERE user_id = ? AND added_on IS NOT NULL",
        )
        .bind(user_id)
        .fetch_all(&state.pool)
        .await?,
    ))
}

pub async fn user_json(
    state: &AppState,
    user_id: &str,
) -> Result<Value, ApiError> {
    let row: Option<(String, Option<String>)> = sqlx::query_as(
        "SELECT username, avatar_url FROM accounts WHERE id = ?",
    )
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    let (username, avatar_url) =
        row.unwrap_or_else(|| (user_id.to_string(), None));
    Ok(json!({"id": user_id, "username": username, "avatar_url": avatar_url}))
}

/// Everyone with access to a server, the owner first. Pending invites have no `added_on`.
pub async fn list(
    state: &AppState,
    server_id: &str,
    created: &str,
) -> Result<Value, ApiError> {
    let owner = state
        .owner()
        .await?
        .ok_or_else(|| ApiError::Forbidden("this Core is not paired".into()))?;
    let mut users = vec![json!({
        "user": user_json(state, &owner.user_id).await?,
        "added_on": created,
        "last_invite_sent": null,
        "permissions": Role::Owner.permissions_json(),
    })];
    let rows: Vec<(String, String, String, Option<String>)> = sqlx::query_as(
        "SELECT user_id, role, last_invite_sent, added_on FROM server_users WHERE server_id = ? ORDER BY last_invite_sent",
    )
    .bind(server_id)
    .fetch_all(&state.pool)
    .await?;
    for (user_id, role, last_invite_sent, added_on) in rows {
        let role = Role::assignable(&role)?;
        users.push(json!({
            "user": user_json(state, &user_id).await?,
            "added_on": added_on,
            "last_invite_sent": last_invite_sent,
            "permissions": role.permissions_json(),
        }));
    }
    Ok(json!(users))
}

/// This Core's key for its own calls to the backend.
async fn backend_key(state: &AppState) -> Result<String, ApiError> {
    state
        .owner()
        .await?
        .and_then(|owner| owner.backend_key)
        .ok_or_else(|| {
            ApiError::Conflict(
                "Pair this Core in the App before inviting people".into(),
            )
        })
}

#[derive(Deserialize)]
struct Invited {
    id: String,
    username: String,
    avatar_url: Option<String>,
}

/// Tell the backend `user` may reach this Core, which also sends them the invite.
async fn send_invite(
    state: &AppState,
    actor: &Account,
    user: &str,
    server_id: &str,
    server_name: &str,
    role: Role,
) -> Result<Account, ApiError> {
    let response = state
        .http
        .post(format!("{}/core/access", state.config.account_api_url))
        .bearer_auth(backend_key(state).await?)
        .json(&json!({
            "user": user,
            "server_id": server_id,
            "server_name": server_name,
            "role": role.stored(),
            "invited_by": actor.id,
        }))
        .send()
        .await
        .map_err(unavailable)?;
    let status = response.status();
    if !status.is_success() {
        #[derive(Deserialize, Default)]
        struct Refused {
            #[serde(default)]
            description: String,
        }
        let reason = response.json::<Refused>().await.unwrap_or_default();
        return Err(match status {
            StatusCode::NOT_FOUND => {
                ApiError::NotFound("No account has that name".into())
            }
            StatusCode::BAD_REQUEST => ApiError::BadRequest(reason.description),
            _ => ApiError::ServiceUnavailable(format!(
                "Amberite answered {status} to the invite"
            )),
        });
    }
    let invited: Invited = response.json().await.map_err(unavailable)?;
    let account = Account {
        id: invited.id,
        username: invited.username,
        avatar_url: invited.avatar_url,
    };
    remember(state, &account).await;
    Ok(account)
}

/// Withdraw `user_id`'s invite to a server at the backend, and its way to this Core when it has
/// no server left here.
async fn withdraw(state: &AppState, user_id: &str, server_id: &str) {
    let remaining: i64 = match sqlx::query_scalar(
        "SELECT COUNT(*) FROM server_users WHERE user_id = ?",
    )
    .bind(user_id)
    .fetch_one(&state.pool)
    .await
    {
        Ok(remaining) => remaining,
        Err(error) => {
            warn!(%error, "could not count an account's servers");
            return;
        }
    };
    let Ok(key) = backend_key(state).await else {
        return;
    };
    // Core has already refused this account, so a backend that cannot be reached only leaves
    // the Core in their list a little longer.
    let sent = state
        .http
        .delete(format!("{}/core/access", state.config.account_api_url))
        .bearer_auth(key)
        .json(&json!({
            "user_id": user_id,
            "server_id": server_id,
            "keep_core": remaining > 0,
        }))
        .send()
        .await
        .and_then(reqwest::Response::error_for_status);
    if let Err(error) = sent {
        warn!(%error, "could not tell Amberite an account lost access");
    }
}

/// Invite an account, by id or username, to a server.
pub async fn invite(
    state: &AppState,
    actor: &Account,
    server_id: &str,
    server_name: &str,
    user: &str,
    role: Role,
) -> Result<(), ApiError> {
    let existing: Option<i64> = sqlx::query_scalar(
        "SELECT 1 FROM server_users su LEFT JOIN accounts a ON a.id = su.user_id          WHERE su.server_id = ? AND (su.user_id = ? OR a.username = ? COLLATE NOCASE)",
    )
    .bind(server_id)
    .bind(user)
    .bind(user)
    .fetch_optional(&state.pool)
    .await?;
    if existing.is_some() {
        return Err(ApiError::Conflict(
            "That account already has access or an invite".into(),
        ));
    }
    let invited =
        send_invite(state, actor, user, server_id, server_name, role).await?;
    sqlx::query(
        "INSERT OR IGNORE INTO server_users (server_id, user_id, role, last_invite_sent) VALUES (?, ?, ?, ?)",
    )
    .bind(server_id)
    .bind(&invited.id)
    .bind(role.stored())
    .bind(now())
    .execute(&state.pool)
    .await?;
    log(
        state,
        server_id,
        &actor.id,
        "user_invited",
        json!({"user_id": invited.id, "permissions": role.permissions_json()}),
    )
    .await;
    Ok(())
}

/// Send a pending invite again. Returns `(sent, cooldown_seconds)`.
pub async fn reinvite(
    state: &AppState,
    actor: &Account,
    server_id: &str,
    server_name: &str,
    user_id: &str,
) -> Result<(bool, Option<i64>), ApiError> {
    let row: Option<(String, String, Option<String>)> = sqlx::query_as(
        "SELECT role, last_invite_sent, added_on FROM server_users WHERE server_id = ? AND user_id = ?",
    )
    .bind(server_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((role, last_invite_sent, None)) = row else {
        return Err(ApiError::NotFound("no pending invite".into()));
    };
    let waited = chrono::DateTime::parse_from_rfc3339(&last_invite_sent)
        .map(|sent| (Utc::now() - sent.with_timezone(&Utc)).num_seconds())
        .unwrap_or(REINVITE_COOLDOWN_SECONDS);
    if waited < REINVITE_COOLDOWN_SECONDS {
        return Ok((false, Some(REINVITE_COOLDOWN_SECONDS - waited)));
    }
    let role = Role::assignable(&role)?;
    send_invite(state, actor, user_id, server_id, server_name, role).await?;
    sqlx::query(
        "UPDATE server_users SET last_invite_sent = ? WHERE server_id = ? AND user_id = ?",
    )
    .bind(now())
    .bind(server_id)
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    log(
        state,
        server_id,
        &actor.id,
        "user_invited",
        json!({"user_id": user_id, "permissions": role.permissions_json()}),
    )
    .await;
    Ok((true, Some(REINVITE_COOLDOWN_SECONDS)))
}

pub async fn set_role(
    state: &AppState,
    actor: &Account,
    server_id: &str,
    user_id: &str,
    role: Role,
) -> Result<(), ApiError> {
    let changed = sqlx::query(
        "UPDATE server_users SET role = ? WHERE server_id = ? AND user_id = ?",
    )
    .bind(role.stored())
    .bind(server_id)
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    if changed.rows_affected() == 0 {
        return Err(ApiError::NotFound(
            "that account has no access to this server".into(),
        ));
    }
    log(
        state,
        server_id,
        &actor.id,
        "user_permission_modified",
        json!({"user_id": user_id, "permissions": role.permissions_json()}),
    )
    .await;
    Ok(())
}

/// Take away an account's access or pending invite. `actor_id` is who did it, which is the
/// account itself when it leaves or declines.
pub async fn remove(
    state: &AppState,
    actor_id: &str,
    server_id: &str,
    user_id: &str,
) -> Result<(), ApiError> {
    let removed: Option<Option<String>> = sqlx::query_scalar(
        "DELETE FROM server_users WHERE server_id = ? AND user_id = ? RETURNING added_on",
    )
    .bind(server_id)
    .bind(user_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some(added_on) = removed else {
        return Err(ApiError::NotFound(
            "that account has no access to this server".into(),
        ));
    };
    withdraw(state, user_id, server_id).await;
    let action = if added_on.is_some() {
        "user_removed"
    } else {
        "user_invite_revoked"
    };
    log(
        state,
        server_id,
        actor_id,
        action,
        json!({"user_id": user_id}),
    )
    .await;
    Ok(())
}

/// The invited account accepts. Returns false when it has no pending invite.
pub async fn accept(
    state: &AppState,
    user_id: &str,
    server_id: &str,
) -> Result<bool, ApiError> {
    let accepted = sqlx::query(
        "UPDATE server_users SET added_on = ? WHERE server_id = ? AND user_id = ? AND added_on IS NULL",
    )
    .bind(now())
    .bind(server_id)
    .bind(user_id)
    .execute(&state.pool)
    .await?;
    Ok(accepted.rows_affected() > 0)
}

/// Forget everyone invited to this Core's servers. Used when the Core is unpaired: its next
/// owner starts with nobody else.
pub async fn forget_everyone(state: &AppState) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM server_users")
        .execute(&state.pool)
        .await?;
    Ok(())
}

/// Record an action in a server's activity log. `metadata` is what the Access tab reads for
/// that action. A log that cannot be written never fails the action itself.
pub async fn log(
    state: &AppState,
    server_id: &str,
    user_id: &str,
    action: &str,
    metadata: Value,
) {
    let stored = sqlx::query(
        "INSERT INTO action_log (server_id, user_id, action, metadata, timestamp) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(server_id)
    .bind(user_id)
    .bind(action)
    .bind(metadata.to_string())
    .bind(now())
    .execute(&state.pool)
    .await;
    if let Err(error) = stored {
        warn!(%error, action, "could not write the activity log");
    }
}

#[derive(Deserialize, Default)]
pub struct LogQuery {
    /// JSON: `{"users": [..], "actions": [..]}`.
    filter: Option<String>,
    limit: Option<i64>,
    offset: Option<i64>,
    order: Option<String>,
    min_datetime: Option<String>,
    max_datetime: Option<String>,
}

#[derive(Deserialize, Default)]
struct LogFilter {
    #[serde(default)]
    users: Vec<String>,
    #[serde(default)]
    actions: Vec<String>,
}

/// One page of a server's activity log, in the shape of Modrinth's `action-log`.
pub async fn read_log(
    state: &Arc<AppState>,
    server_id: &str,
    query: LogQuery,
) -> Result<Value, ApiError> {
    let filter: LogFilter = match &query.filter {
        Some(filter) => serde_json::from_str(filter)
            .map_err(|_| ApiError::BadRequest("invalid filter".into()))?,
        None => LogFilter::default(),
    };
    let limit = query.limit.unwrap_or(100).clamp(1, 500);
    let offset = query.offset.unwrap_or(0).max(0);

    let mut sql = sqlx::QueryBuilder::new(
        "SELECT user_id, action, metadata, timestamp FROM action_log WHERE server_id = ",
    );
    sql.push_bind(server_id);
    for (column, values) in
        [("user_id", &filter.users), ("action", &filter.actions)]
    {
        if values.is_empty() {
            continue;
        }
        sql.push(format!(" AND {column} IN ("));
        let mut list = sql.separated(", ");
        for value in values {
            list.push_bind(value);
        }
        sql.push(")");
    }
    if let Some(min) = &query.min_datetime {
        sql.push(" AND timestamp >= ").push_bind(min);
    }
    if let Some(max) = &query.max_datetime {
        sql.push(" AND timestamp <= ").push_bind(max);
    }
    sql.push(if query.order.as_deref() == Some("asc") {
        " ORDER BY id ASC"
    } else {
        " ORDER BY id DESC"
    });
    // One more than asked for says whether another page exists.
    sql.push(" LIMIT ").push_bind(limit + 1);
    sql.push(" OFFSET ").push_bind(offset);
    let mut rows: Vec<(String, String, String, String)> =
        sql.build_query_as().fetch_all(&state.pool).await?;
    let more = rows.len() as i64 > limit;
    rows.truncate(limit as usize);

    let mut users = serde_json::Map::new();
    let mut addons = serde_json::Map::new();
    let mut versions = serde_json::Map::new();
    let mut data = Vec::new();
    for (user_id, action, metadata, timestamp) in rows {
        let metadata: Value =
            serde_json::from_str(&metadata).unwrap_or(Value::Null);
        let target = metadata["user_id"].as_str();
        for id in [Some(user_id.as_str()), target].into_iter().flatten() {
            if !users.contains_key(id) {
                users.insert(id.to_string(), user_json(state, id).await?);
            }
        }
        // Content actions carry the names they were logged with, so they read the same after
        // the content is gone.
        for addon in metadata["addons"].as_array().into_iter().flatten() {
            if let Some(id) = addon["addon_id"].as_str() {
                addons.insert(
                    id.to_string(),
                    json!({"title": addon["title"], "slug": null, "icon_url": null}),
                );
            }
            if let Some(id) = addon["version_id"].as_str() {
                versions.insert(
                    id.to_string(),
                    json!({"name": addon["version_number"], "version_number": addon["version_number"]}),
                );
            }
        }
        data.push(json!({
            "actor": {"type": "user", "user_id": user_id},
            "action": {"action": action, "metadata": metadata},
            "server_id": server_id,
            "world_id": null,
            "timestamp": timestamp,
        }));
    }
    Ok(json!({
        "next_offset": more.then_some(offset + limit),
        "data": data,
        "users": users,
        "addons": addons,
        "versions": versions,
    }))
}

pub async fn clear_log(
    state: &AppState,
    server_id: &str,
) -> Result<(), ApiError> {
    sqlx::query("DELETE FROM action_log WHERE server_id = ?")
        .bind(server_id)
        .execute(&state.pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{scope, Role};

    #[test]
    fn roles_send_the_numbers_the_app_reads() {
        // The App reads these as signed 64-bit numbers, which JavaScript only keeps exact
        // because each one is a power of two.
        assert_eq!(Role::Owner.permissions_json(), -32768);
        assert_eq!(Role::Editor.permissions_json(), -(1 << 57));
        assert_eq!(Role::Viewer.permissions_json(), -(1 << 62));
    }

    #[test]
    fn a_viewer_can_start_the_server_and_nothing_else() {
        let viewer = Role::Viewer.permissions();
        assert_ne!(viewer & scope::POWER_ACTIONS, 0);
        for denied in [
            scope::EXEC_COMMANDS,
            scope::FILES_WRITE,
            scope::SETUP,
            scope::BACKUPS,
            scope::ADVANCED,
            scope::RESET_SERVER,
            scope::MANAGE_USERS,
        ] {
            assert_eq!(viewer & denied, 0);
        }
        let editor = Role::Editor.permissions();
        assert_eq!(editor & scope::MANAGE_USERS, 0);
        assert_eq!(editor & scope::RESET_SERVER, 0);
        assert_ne!(editor & scope::FILES_WRITE, 0);
    }
}
