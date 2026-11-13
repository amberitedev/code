//! Per-server roles: who besides the owner may see or manage each server (see
//! `contracts::core::ListMembers` and the routes after it).
//!
//! A dangerous action carries a fresh ticket in the `X-Amberite-Ticket` header, so every route
//! asks for it the same way whatever its method and body. Invites and removals are also owed to
//! the backend, whose directory says who may be told where this Core is.

use std::sync::Arc;

use axum::{extract::State, http::HeaderMap};
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    backend::cores::{AccessGrant, AccessRevocation},
    core::{
        Action, ChangeRole, CoreRole, CoreUser, GetMyAccess, InviteMember,
        ListMembers, Member, MyAccess, RemoveMember, ServerAccess, ServerRole,
    },
    server::Req,
};
use sqlx::SqliteConnection;

use super::{CoreState, internal};
use crate::{
    access::{
        Caller, Dangerous, Permission, activity, fresh_ticket, now,
        owed::OwedCall, require_owner, sync::owe,
    },
    servers::ServerError,
    state::servers::adapters::sqlite::server_rows,
};

/// People with a role on any server of this Core, at most. The backend's directory holds no
/// more (`MAX_DIRECTORY` in `apps/backend/src/cores/index.ts`), and a directory past it could
/// never be replaced there. The two must stay equal.
pub const MAX_DIRECTORY: i64 = 1000;

pub async fn me(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetMyAccess>,
) -> Result<Json<MyAccess>, ApiError> {
    let Some(user_id) = caller.user_id() else {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "The local CLI is this Core's owner and has no account here.",
        ));
    };
    let servers: Vec<(String, CoreRole)> = if caller.is_owner() {
        Vec::new()
    } else {
        sqlx::query_as(
            "SELECT server_id, role FROM access WHERE user_id = ? ORDER BY server_id",
        )
        .bind(user_id)
        .fetch_all(&state.db)
        .await.map_err(internal)?
    };
    Ok(Json(MyAccess {
        user: user(&state.db, user_id).await.map_err(internal)?,
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

pub async fn list(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<ListMembers>,
) -> Result<Json<ServerAccess>, ApiError> {
    let server_id = request.params.id;
    state
        .access
        .require(&caller, &server_id, Permission::Owner)
        .await?;
    server(&state, &server_id).await?;
    let owner = match owner_id(&state.db).await.map_err(internal)? {
        Some(id) => Some(user(&state.db, &id).await.map_err(internal)?),
        None => None,
    };
    let rows: Vec<String> = sqlx::query_scalar(
        "SELECT user_id FROM access WHERE server_id = ? ORDER BY added_at, user_id",
    )
    .bind(&server_id)
    .fetch_all(&state.db)
    .await.map_err(internal)?;
    let mut members = Vec::new();
    for user_id in rows {
        members.push(member(&state.db, &server_id, &user_id).await?);
    }
    Ok(Json(ServerAccess { owner, members }))
}

pub async fn invite(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    Req {
        params,
        body: invite,
        ..
    }: Req<InviteMember>,
) -> Result<Json<Member>, ApiError> {
    let server_id = params.id;
    let server = server(&state, &server_id).await?;
    let CoreUser {
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
            ErrorCode::InvalidInput,
            "Invite someone by their account id and name.",
        ));
    }
    if owner_id(&state.db).await.map_err(internal)?.as_deref()
        == Some(user_id.as_str())
    {
        return Err(ApiError::new(
            ErrorCode::Conflict,
            "The owner already has every role on every server.",
        ));
    }

    let mut tx = state.db.begin().await.map_err(internal)?;
    let (known, people): (bool, i64) = sqlx::query_as(
        "SELECT EXISTS (SELECT 1 FROM access WHERE user_id = ?), \
         (SELECT COUNT(DISTINCT user_id) FROM access)",
    )
    .bind(&user_id)
    .fetch_one(&mut *tx)
    .await
    .map_err(internal)?;
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
    .await.map_err(internal)?;
    sqlx::query(
        "INSERT INTO access (server_id, user_id, role, added_at) VALUES (?, ?, ?, ?) \
         ON CONFLICT (server_id, user_id) DO UPDATE SET role = excluded.role",
    )
    .bind(&server_id)
    .bind(&user_id)
    .bind(invite.role)
    .bind(now())
    .execute(&mut *tx)
    .await.map_err(internal)?;
    activity::record(
        &mut tx,
        caller.user_id(),
        Some(&server_id),
        &Action::Invited {
            user_id: user_id.clone(),
            role: invite.role,
        },
    )
    .await
    .map_err(internal)?;
    owe(
        &mut tx,
        &OwedCall::Grant(AccessGrant {
            user: user_id.clone(),
            server_id: server_id.clone(),
            server_name: server.name,
            role: invite.role,
        }),
    )
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    state.access.owed.notify_one();
    state.access.changed(&user_id);
    Ok(Json(member(&state.db, &server_id, &user_id).await?))
}

pub async fn change_role(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    Req {
        params,
        body: change,
        ..
    }: Req<ChangeRole>,
) -> Result<Json<Member>, ApiError> {
    let (server_id, user_id) = (params.id, params.user_id);
    let mut tx = state.db.begin().await.map_err(internal)?;
    let changed = sqlx::query(
        "UPDATE access SET role = ? WHERE server_id = ? AND user_id = ?",
    )
    .bind(change.role)
    .bind(&server_id)
    .bind(&user_id)
    .execute(&mut *tx)
    .await
    .map_err(internal)?;
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
    .await
    .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    // An editor made a viewer loses the console at once.
    state.access.changed(&user_id);
    Ok(Json(member(&state.db, &server_id, &user_id).await?))
}

/// Every session the user has on this Core ends, and with them their console streams; their App
/// signs in again with a ticket for what is left.
pub async fn remove(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    headers: HeaderMap,
    request: Req<RemoveMember>,
) -> Result<Empty, ApiError> {
    let (server_id, user_id) = (request.params.id, request.params.user_id);
    if caller.user_id() != Some(user_id.as_str()) {
        require_owner(&caller)?;
        fresh_ticket(&state, &caller, &headers).await?;
    }
    let mut tx = state.db.begin().await.map_err(internal)?;
    let removed =
        sqlx::query("DELETE FROM access WHERE server_id = ? AND user_id = ?")
            .bind(&server_id)
            .bind(&user_id)
            .execute(&mut *tx)
            .await
            .map_err(internal)?;
    if removed.rows_affected() == 0 {
        return Err(not_a_member());
    }
    sqlx::query("DELETE FROM sessions WHERE user_id = ?")
        .bind(&user_id)
        .execute(&mut *tx)
        .await
        .map_err(internal)?;
    activity::record(
        &mut tx,
        caller.user_id(),
        Some(&server_id),
        &Action::Removed {
            user_id: user_id.clone(),
        },
    )
    .await
    .map_err(internal)?;
    owe_removal(&mut tx, &user_id, &server_id)
        .await
        .map_err(internal)?;
    tx.commit().await.map_err(internal)?;
    state.access.owed.notify_one();
    state.access.changed(&user_id);
    Ok(Empty)
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
        &OwedCall::Remove(AccessRevocation {
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
    state: &CoreState,
    id: &str,
) -> Result<server_rows::ServerRow, ApiError> {
    Ok(server_rows::get(&state.db, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.into()))?)
}

/// A user as Core last heard of them. One it never heard of is named by their id.
async fn user(db: &sqlx::SqlitePool, id: &str) -> sqlx::Result<CoreUser> {
    let row: Option<(String, Option<String>)> =
        sqlx::query_as("SELECT name, avatar_url FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(db)
            .await?;
    let (name, avatar_url) = row.unwrap_or_else(|| (id.into(), None));
    Ok(CoreUser {
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
    let (role, added_at): (CoreRole, i64) = sqlx::query_as(
        "SELECT role, added_at FROM access WHERE server_id = ? AND user_id = ?",
    )
    .bind(server_id)
    .bind(user_id)
    .fetch_optional(db)
    .await
    .map_err(internal)?
    .ok_or_else(not_a_member)?;
    Ok(Member {
        user: user(db, user_id).await.map_err(internal)?,
        role,
        added_at,
    })
}
