//! Per-server roles: who besides the owner may see or manage each server.
//!
//! - `GET /access/me`: the caller's own access. [`MyAccess`].
//! - `GET /servers/{id}/access`: the owner and members of a server. [`ServerAccess`].
//! - `POST /servers/{id}/access`: invite someone, which grants the role at once. [`Invite`] in,
//!   [`Member`] out. Owner only, dangerous.
//! - `PATCH /servers/{id}/access/{user_id}`: change a role. [`ChangeRole`] in, [`Member`] out.
//!   Owner only, dangerous.
//! - `DELETE /servers/{id}/access/{user_id}`: remove someone; their sessions and console streams
//!   end. Owner only, dangerous.
//!
//! A dangerous action carries a fresh ticket in the `X-Amberite-Ticket` header, so every route
//! asks for it the same way whatever its method and body. Invites and removals are also owed to
//! the backend, whose directory says who may be told where this Core is.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use serde::{Deserialize, Serialize};
use sqlx::SqliteConnection;
use ts_rs::TS;
use uuid::Uuid;

use super::{
    AppState,
    activity::Action,
    backend::{Grant, OwedCall, Remove},
    error::{ApiError, ErrorCode},
};
use crate::{
    access::{Caller, Dangerous, Level, activity, now, sync::owe},
    servers::{ServerError, records},
};

/// People with a role on any server of this Core, at most. The backend's directory holds no
/// more (`MAX_DIRECTORY` in `apps/backend/src/cores/index.ts`), and a directory past it could
/// never be replaced there. The two must stay equal.
pub const MAX_DIRECTORY: i64 = 1000;

/// A role on one server. The owner has every role on every server and is never a member.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Serialize,
    Deserialize,
    TS,
    sqlx::Type,
    clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum Role {
    /// Sees status, players, content, and backups. No console: it carries chat and player IPs.
    Viewer,
    /// Everything a viewer can, plus running, console, files, settings, and backups. Runs code on
    /// the host machine.
    Editor,
}

/// A user as their last ticket named them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
pub struct User {
    pub id: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Member {
    pub user: User,
    pub role: Role,
    /// Unix seconds.
    pub added_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ServerAccess {
    /// Null until the Core is paired.
    pub owner: Option<User>,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, Deserialize, TS)]
pub struct Invite {
    /// Shown until the invited user signs in and their own ticket names them.
    pub user: User,
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ChangeRole {
    pub role: Role,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct MyAccess {
    pub user: User,
    /// The owner of this Core, with every role on every server.
    pub owner: bool,
    /// Empty for the owner.
    pub servers: Vec<ServerRole>,
}

#[derive(Debug, Clone, Serialize, TS)]
pub struct ServerRole {
    pub server_id: Uuid,
    pub role: Role,
}

/// `GET /access/me`.
pub async fn me(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<MyAccess>, ApiError> {
    let Some(user_id) = caller.user_id() else {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "The local CLI is this Core's owner and has no account here.",
        ));
    };
    let servers: Vec<(String, Role)> = if caller.is_owner() {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT server_id, role FROM access WHERE user_id = ? ORDER BY server_id",
        )
        .bind(user_id)
        .fetch_all(&state.db)
        .await?
    };
    Ok(Json(MyAccess {
        user: user(&state.db, user_id).await?,
        owner: caller.is_owner(),
        servers: servers
            .into_iter()
            .filter_map(|(server_id, role)| {
                Some(ServerRole {
                    server_id: server_id.parse().ok()?,
                    role,
                })
            })
            .collect(),
    }))
}

/// `GET /servers/{id}/access`. Owner only.
pub async fn list(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<String>,
) -> Result<Json<ServerAccess>, ApiError> {
    caller.require(&state.db, &server_id, Level::Owner).await?;
    server(&state, &server_id).await?;
    let owner = match owner_id(&state.db).await? {
        Some(id) => Some(user(&state.db, &id).await?),
        None => None,
    };
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT user_id FROM access WHERE server_id = ? ORDER BY added_at, user_id",
    )
    .bind(&server_id)
    .fetch_all(&state.db)
    .await?;
    let mut members = Vec::new();
    for user_id in rows {
        members.push(member(&state.db, &server_id, &user_id).await?);
    }
    Ok(Json(ServerAccess { owner, members }))
}

/// `POST /servers/{id}/access`. Inviting someone who already has a role sets the new one and
/// notifies them again.
pub async fn invite(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    Path(server_id): Path<String>,
    Json(invite): Json<Invite>,
) -> Result<Json<Member>, ApiError> {
    let server = server(&state, &server_id).await?;
    let User {
        id: user_id,
        name,
        avatar_url,
    } = invite.user;
    if user_id.is_empty()
        || user_id.len() > 64
        || name.is_empty()
        || name.chars().count() > 64
    {
        return Err(ApiError::new(
            ErrorCode::InvalidRequest,
            "Invite someone by their account id and name.",
        ));
    }
    if owner_id(&state.db).await?.as_deref() == Some(user_id.as_str()) {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "The owner already has every role on every server.",
        ));
    }

    let mut tx = state.db.begin().await?;
    let (known, people): (bool, i64) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM access WHERE user_id = ?), \
         (SELECT COUNT(DISTINCT user_id) FROM access)",
    )
    .bind(&user_id)
    .fetch_one(&mut *tx)
    .await?;
    if !known && people >= MAX_DIRECTORY {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            format!(
                "This Core already has {MAX_DIRECTORY} people with access, the most it can have. Remove someone first."
            ),
        ));
    }
    // A name their own ticket gave is newer than the one in the invite.
    sqlx::query(
        "INSERT INTO users (id, name, avatar_url, updated_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT (id) DO NOTHING",
    )
    .bind(&user_id)
    .bind(&name)
    .bind(&avatar_url)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO access (server_id, user_id, role, added_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT (server_id, user_id) DO UPDATE SET role = excluded.role",
    )
    .bind(&server_id)
    .bind(&user_id)
    .bind(invite.role)
    .bind(now())
    .execute(&mut *tx)
    .await?;
    activity::record(
        &mut tx,
        caller.user_id(),
        Some(&server_id),
        &Action::Invited {
            user_id: user_id.clone(),
            role: invite.role,
        },
    )
    .await?;
    owe(
        &mut tx,
        &OwedCall::Grant(Grant {
            user: user_id.clone(),
            server_id: server_id.clone(),
            server_name: server.name,
            role: invite.role,
        }),
    )
    .await?;
    tx.commit().await?;
    state.access.owed.notify_one();
    state.access.changed(&user_id);
    Ok(Json(member(&state.db, &server_id, &user_id).await?))
}

/// `PATCH /servers/{id}/access/{user_id}`.
pub async fn change_role(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    Path((server_id, user_id)): Path<(String, String)>,
    Json(change): Json<ChangeRole>,
) -> Result<Json<Member>, ApiError> {
    let mut tx = state.db.begin().await?;
    let changed = sqlx::query(
        "UPDATE access SET role = ? WHERE server_id = ? AND user_id = ?",
    )
    .bind(change.role)
    .bind(&server_id)
    .bind(&user_id)
    .execute(&mut *tx)
    .await?;
    if changed.rows_affected() == 0 {
        return Err(not_a_member());
    }
    activity::record(
        &mut tx,
        caller.user_id(),
        Some(&server_id),
        &Action::RoleChanged {
            user_id: user_id.clone(),
            role: change.role,
        },
    )
    .await?;
    tx.commit().await?;
    // An editor made a viewer loses the console at once.
    state.access.changed(&user_id);
    Ok(Json(member(&state.db, &server_id, &user_id).await?))
}

/// `DELETE /servers/{id}/access/{user_id}`. Every session the user has on this Core ends, and
/// with them their console streams; their App signs in again with a ticket for what is left.
pub async fn remove(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<AppState>>,
    Path((server_id, user_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let mut tx = state.db.begin().await?;
    let removed =
        sqlx::query("DELETE FROM access WHERE server_id = ? AND user_id = ?")
            .bind(&server_id)
            .bind(&user_id)
            .execute(&mut *tx)
            .await?;
    if removed.rows_affected() == 0 {
        return Err(not_a_member());
    }
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&user_id)
        .execute(&mut *tx)
        .await?;
    activity::record(
        &mut tx,
        caller.user_id(),
        Some(&server_id),
        &Action::Removed {
            user_id: user_id.clone(),
        },
    )
    .await?;
    owe_removal(&mut tx, &user_id, &server_id).await?;
    tx.commit().await?;
    state.access.owed.notify_one();
    state.access.changed(&user_id);
    Ok(StatusCode::NO_CONTENT)
}

/// Owe the backend `user_id` losing `server_id`, once their row is gone. They stay in the
/// directory while they have another server here.
pub async fn owe_removal(
    conn: &mut SqliteConnection,
    user_id: &str,
    server_id: &str,
) -> sqlx::Result<()> {
    let keep_core: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM access WHERE user_id = ?)",
    )
    .bind(user_id)
    .fetch_one(&mut *conn)
    .await?;
    owe(
        conn,
        &OwedCall::Remove(Remove {
            user_id: user_id.into(),
            server_id: server_id.into(),
            keep_core,
        }),
    )
    .await
}

fn not_a_member() -> ApiError {
    ApiError::new(ErrorCode::NotFound, "That user has no role on this server.")
}

async fn owner_id(db: &sqlx::SqlitePool) -> sqlx::Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT owner_user_id FROM core WHERE id = 1")
            .fetch_optional(db)
            .await?
            .flatten(),
    )
}

async fn server(
    state: &AppState,
    id: &str,
) -> Result<records::ServerRow, ApiError> {
    Ok(records::get(&state.db, id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.into()))?)
}

/// A user as Core last heard of them. One it never heard of is named by their id.
async fn user(db: &sqlx::SqlitePool, id: &str) -> sqlx::Result<User> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT name, avatar_url FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
    let (name, avatar_url) = row.unwrap_or_else(|| (id.into(), None));
    Ok(User {
        id: id.into(),
        name,
        avatar_url,
    })
}

async fn member(
    db: &sqlx::SqlitePool,
    server_id: &str,
    user_id: &str,
) -> Result<Member, ApiError> {
    let (role, added_at): (Role, i64) = sqlx::query_as(
        "SELECT role, added_at FROM access WHERE server_id = ? AND user_id = ?",
    )
    .bind(server_id)
    .bind(user_id)
    .fetch_optional(db)
    .await?
    .ok_or_else(not_a_member)?;
    Ok(Member {
        user: user(db, user_id).await?,
        role,
        added_at,
    })
}
