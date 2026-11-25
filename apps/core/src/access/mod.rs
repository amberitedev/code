//! Who can do what: tickets, sessions, the local socket, the activity log, and
//! backend sync. A Core answers its owner only. The request and response shapes are in
//! `api::sessions`, `api::activity`, and, for the backend, `contracts::backend::cores`.
//!
//! Every route takes a [`Caller`] and checks it: [`CoreAccess::require`] for one server,
//! [`require_owner`] for the Core itself, and [`Dangerous`] for the owner's actions that need a
//! fresh ticket.

pub mod activity;
pub mod keys;
pub mod local;
pub mod owed;
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
use contracts::{ApiError, ErrorCode, core::TICKET_HEADER};
pub use engine::access::Caller;
use engine::access::{Access, BoxFuture};
use sqlx::SqlitePool;
use tokio::sync::{Notify, broadcast};

use crate::{
    api::{CoreState, Shared, internal},
    db,
};

/// `/events` sockets one session may hold.
const STREAMS_PER_SESSION: usize = 4;
const SIGN_INS_PER_MINUTE: u32 = 10;

/// Unix seconds.
pub fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Unix `seconds` as the wire's time.
pub fn at(seconds: i64) -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::from_timestamp(seconds, 0).unwrap_or_default()
}

/// Access state shared by every route, and the engine's [`Access`] on this Core.
pub struct CoreAccess {
    db: SqlitePool,
    pub keys: keys::Keys,
    /// Users whose sessions changed. `/events` sockets listen, and close once their session
    /// ended.
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

impl CoreAccess {
    pub fn new(db: SqlitePool, keys: keys::Keys) -> Self {
        Self {
            db,
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

    /// Hold one of a session's `/events` sockets until the guard drops. `None` at the cap.
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

    /// Whether `caller`'s session is still live: not ended, expired, or idle past
    /// `sessions::IDLE_S`. The local CLI's always is.
    pub async fn revalidate(&self, caller: &Caller) -> bool {
        let Caller::Session { id, .. } = caller else {
            return true;
        };
        let live = sqlx::query(
            "SELECT 1 FROM sessions WHERE id = ?1 AND expires_at > ?2 AND last_used_at > ?2 - ?3",
        )
        .bind(id)
        .bind(now())
        .bind(sessions::IDLE_S)
        .fetch_optional(&self.db)
        .await;
        matches!(live, Ok(Some(_)))
    }
}

/// One open `/events` socket of a session. The local CLI's are not counted.
pub struct StreamGuard(Option<(Arc<CoreAccess>, String)>);

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

pub fn require_owner(caller: &Caller) -> Result<(), ApiError> {
    if caller.is_owner() {
        Ok(())
    } else {
        Err(ApiError::new(
            ErrorCode::Forbidden,
            "Only this Core's owner can do that.",
        ))
    }
}

impl CoreAccess {
    /// Every server for the owner. Anyone else gets the same 404 as for a server that does not
    /// exist. Every server-scoped route and topic goes through it, so `server_id` names a server
    /// in `core.db` before it reaches a path.
    pub async fn require(
        &self,
        caller: &Caller,
        server_id: &str,
    ) -> Result<(), ApiError> {
        let exists = caller.is_owner()
            && sqlx::query("SELECT 1 FROM servers WHERE id = ?1")
                .bind(server_id)
                .fetch_optional(&self.db)
                .await
                .map_err(internal)?
                .is_some();
        if exists {
            Ok(())
        } else {
            Err(ApiError::new(
                ErrorCode::NotFound,
                format!("There is no server {server_id}."),
            ))
        }
    }
}

impl Access for CoreAccess {
    fn caller<'a>(
        &'a self,
        request: &'a Parts,
    ) -> BoxFuture<'a, Result<Caller, ApiError>> {
        Box::pin(async move {
            if request.extensions.get::<local::Local>().is_some() {
                return Ok(Caller::Local);
            }
            let token = request
                .headers
                .get(AUTHORIZATION)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.strip_prefix("Bearer "))
                .ok_or_else(|| {
                    ApiError::new(
                        ErrorCode::InvalidCredentials,
                        "Sign in to this Core with a ticket first.",
                    )
                })?;
            sessions::authenticate(&self.db, token, now()).await
        })
    }

    /// Without a server, only the owner gets in.
    fn require<'a>(
        &'a self,
        caller: &'a Caller,
        instance: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), ApiError>> {
        Box::pin(async move {
            match instance {
                Some(server_id) => Self::require(self, caller, server_id).await,
                None => require_owner(caller),
            }
        })
    }
}

/// The owner doing something dangerous: deleting a server; ending another device's session;
/// updating or rolling back Core; removing Core from its account; restoring or deleting a backup,
/// or keeping fewer scheduled ones. Over HTTPS this needs a fresh ticket (see [`fresh_ticket`]);
/// the local CLI needs none.
pub struct Dangerous(pub Caller);

impl FromRequestParts<Shared> for Dangerous {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Shared,
    ) -> Result<Self, ApiError> {
        let caller = Caller::from_request_parts(parts, state).await?;
        Self::check(&state.0, caller, &parts.headers).await
    }
}

impl Dangerous {
    /// The extractor's checks, for a route where only some changes are dangerous, such as
    /// keeping fewer scheduled backups.
    pub async fn check(
        state: &CoreState,
        caller: Caller,
        headers: &HeaderMap,
    ) -> Result<Self, ApiError> {
        require_owner(&caller)?;
        fresh_ticket(state, &caller, headers).await?;
        Ok(Self(caller))
    }
}

/// Check and use up the ticket in [`TICKET_HEADER`]: under five minutes old, never used, and for
/// the session's own user. A stolen session alone cannot pass this. The local CLI always does.
pub async fn fresh_ticket(
    state: &CoreState,
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
    let core_id = db::core_id(&state.db).await.map_err(internal)?;
    let claims =
        tickets::verify(&state.access.keys, ticket, &core_id, now).await?;
    if claims.sub != user_id {
        return Err(ApiError::new(
            ErrorCode::TicketInvalid,
            "The ticket is for someone else.",
        ));
    }
    let mut conn = state.db.acquire().await.map_err(internal)?;
    tickets::spend(&mut conn, &claims, now).await
}
