//! Who can do what: tickets, sessions, per-server roles, the local socket, the activity log, and
//! backend sync. The request and response shapes are in `api::access`, `api::sessions`,
//! `api::activity`, and `api::backend`.
//!
//! Every route takes a [`Caller`] and checks it: [`Caller::require`] for a role on one server,
//! [`Caller::require_owner`] for the Core itself, and [`Dangerous`] for the owner's actions that
//! need a fresh ticket.

pub mod activity;
pub mod keys;
pub mod local;
pub mod pairing;
pub mod sessions;
pub mod sync;
pub mod tickets;

use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    extract::FromRequestParts,
    http::{HeaderMap, header::AUTHORIZATION, request::Parts},
};
use tokio::sync::{Notify, broadcast};

use crate::{
    api::{
        AppState,
        access::Role,
        error::{ApiError, ErrorCode},
    },
    db,
};

/// The header a dangerous action carries its fresh ticket in.
pub const TICKET_HEADER: &str = "x-amberite-ticket";
/// Live console streams one session may hold.
const STREAMS_PER_SESSION: usize = 4;
const SIGN_INS_PER_MINUTE: u32 = 10;

/// Unix seconds.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Access state shared by every route.
pub struct Access {
    pub keys: keys::Keys,
    /// Users whose sessions or roles changed. Live console streams listen and close when their
    /// session or role is gone.
    pub changed: broadcast::Sender<Changed>,
    /// Wakes backend sync when a call is owed.
    pub owed: Arc<Notify>,
    /// Asks the pairing loop for a fresh code (`copal pair`).
    pub new_code: Notify,
    streams: Mutex<HashMap<String, usize>>,
    sign_ins: Mutex<HashMap<IpAddr, (Instant, u32)>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Changed {
    User(String),
    /// Every session ended: the Core was un-paired.
    Everyone,
}

impl Access {
    pub fn new(keys: keys::Keys) -> Self {
        Self {
            keys,
            changed: broadcast::channel(64).0,
            owed: Arc::new(Notify::new()),
            new_code: Notify::new(),
            streams: Mutex::default(),
            sign_ins: Mutex::default(),
        }
    }

    /// Tell live streams that `user`'s access changed.
    pub fn changed(&self, user: &str) {
        let _ = self.changed.send(Changed::User(user.into()));
    }

    /// Count a sign-in from `address`. False past ten in a minute.
    pub fn allow_sign_in(&self, address: IpAddr) -> bool {
        let mut sign_ins = self.sign_ins.lock().expect("not poisoned");
        sign_ins
            .retain(|_, (start, _)| start.elapsed() < Duration::from_secs(60));
        let (_, count) = sign_ins.entry(address).or_insert((Instant::now(), 0));
        *count += 1;
        *count <= SIGN_INS_PER_MINUTE
    }

    /// Hold one of a session's console streams until the guard drops. `None` at the cap.
    pub fn open_stream(
        self: &Arc<Self>,
        caller: &Caller,
    ) -> Option<StreamGuard> {
        let Caller::Session { id, .. } = caller else {
            return Some(StreamGuard(None));
        };
        let mut streams = self.streams.lock().expect("not poisoned");
        let open = streams.entry(id.clone()).or_default();
        if *open >= STREAMS_PER_SESSION {
            return None;
        }
        *open += 1;
        Some(StreamGuard(Some((Arc::clone(self), id.clone()))))
    }
}

/// One open console stream of a session. The local CLI's are not counted.
pub struct StreamGuard(Option<(Arc<Access>, String)>);

impl Drop for StreamGuard {
    fn drop(&mut self) {
        if let Some((access, id)) = &self.0 {
            let mut streams = access.streams.lock().expect("not poisoned");
            if let Some(open) = streams.get_mut(id) {
                *open -= 1;
                if *open == 0 {
                    streams.remove(id);
                }
            }
        }
    }
}

/// The peer address of an HTTPS connection, for limits. The local socket has none.
#[derive(Debug, Clone, Copy)]
pub struct Peer(pub IpAddr);

/// What a caller may do on one server. The owner has everything on every server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Level {
    Viewer,
    Editor,
    Owner,
}

impl From<Role> for Level {
    fn from(role: Role) -> Self {
        match role {
            Role::Viewer => Self::Viewer,
            Role::Editor => Self::Editor,
        }
    }
}

/// Who made a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// The CLI over the local socket: the owner, with no token and no ticket.
    Local,
    Session {
        /// The session's public id.
        id: String,
        user_id: String,
        /// The user is this Core's owner.
        owner: bool,
    },
}

impl Caller {
    /// The signed-in user. `None` for the local CLI.
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Session { user_id, .. } => Some(user_id),
        }
    }

    pub fn is_owner(&self) -> bool {
        matches!(self, Self::Local | Self::Session { owner: true, .. })
    }

    pub fn require_owner(&self) -> Result<(), ApiError> {
        if self.is_owner() {
            Ok(())
        } else {
            Err(ApiError::new(
                ErrorCode::Forbidden,
                "Only this Core's owner can do that.",
            ))
        }
    }

    /// The caller's level on `server_id`, `None` without a role there.
    pub async fn level(
        &self,
        db: &sqlx::SqlitePool,
        server_id: &str,
    ) -> sqlx::Result<Option<Level>> {
        let Self::Session { user_id, owner, .. } = self else {
            return Ok(Some(Level::Owner));
        };
        if *owner {
            return Ok(Some(Level::Owner));
        }
        let role: Option<Role> = sqlx::query_scalar(
            "SELECT role FROM access WHERE server_id = ? AND user_id = ?",
        )
        .bind(server_id)
        .bind(user_id)
        .fetch_optional(db)
        .await?;
        Ok(role.map(Level::from))
    }

    /// Require at least `level` on `server_id`. A caller with no role there gets the same 404 as
    /// for a server that does not exist: friends never learn about the other servers.
    pub async fn require(
        &self,
        db: &sqlx::SqlitePool,
        server_id: &str,
        level: Level,
    ) -> Result<Level, ApiError> {
        match self.level(db, server_id).await? {
            None => Err(ApiError::new(
                ErrorCode::NotFound,
                format!("There is no server {server_id}."),
            )),
            Some(has) if has < level => Err(ApiError::new(
                ErrorCode::Forbidden,
                match level {
                    Level::Editor => "Viewers cannot do that on this server.",
                    _ => "Only this Core's owner can do that.",
                },
            )),
            Some(has) => Ok(has),
        }
    }
}

impl FromRequestParts<Arc<AppState>> for Caller {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, ApiError> {
        if parts.extensions.get::<local::Local>().is_some() {
            return Ok(Self::Local);
        }
        let token = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.strip_prefix("Bearer "))
            .ok_or_else(|| {
                ApiError::new(
                    ErrorCode::Unauthorized,
                    "Sign in to this Core with a ticket first.",
                )
            })?;
        sessions::authenticate(&state.db, token, now()).await
    }
}

/// The owner doing something dangerous: inviting, removing, or changing a role; deleting a
/// server; revoking someone else's session; updating or rolling back Core; removing Core from
/// its account; restoring or deleting a backup, or keeping fewer scheduled ones. Over HTTPS this
/// needs a fresh ticket (see [`fresh_ticket`]); the local CLI needs none.
pub struct Dangerous(pub Caller);

impl FromRequestParts<Arc<AppState>> for Dangerous {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, ApiError> {
        let caller = Caller::from_request_parts(parts, state).await?;
        Self::check(state, caller, &parts.headers).await
    }
}

impl Dangerous {
    /// The extractor's checks, for a route where only some changes are dangerous, such as
    /// keeping fewer scheduled backups.
    pub async fn check(
        state: &AppState,
        caller: Caller,
        headers: &HeaderMap,
    ) -> Result<Self, ApiError> {
        caller.require_owner()?;
        fresh_ticket(state, &caller, headers).await?;
        Ok(Self(caller))
    }
}

/// Check and use up the ticket in [`TICKET_HEADER`]: under five minutes old, never used, and for
/// the session's own user. A stolen session alone cannot pass this. The local CLI always does.
pub async fn fresh_ticket(
    state: &AppState,
    caller: &Caller,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let Some(user_id) = caller.user_id() else {
        return Ok(());
    };
    let ticket = headers
        .get(TICKET_HEADER)
        .and_then(|value| value.to_str().ok())
        .ok_or_else(|| {
            ApiError::new(
                ErrorCode::TicketRequired,
                "This needs a fresh ticket in the X-Amberite-Ticket header.",
            )
        })?;
    let now = now();
    let core_id = db::core_id(&state.db).await?;
    let claims =
        tickets::verify(&state.access.keys, ticket, &core_id, now).await?;
    if claims.sub != user_id {
        return Err(ApiError::new(
            ErrorCode::TicketInvalid,
            "The ticket is for someone else.",
        ));
    }
    let mut conn = state.db.acquire().await?;
    tickets::spend(&mut conn, &claims, now).await
}
