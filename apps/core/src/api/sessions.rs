//! Sessions: one per device per Core, made from a ticket (see `contracts::core::SignIn`). The
//! token goes in `Authorization: Bearer` on every other route. Core stores only its hash.

use std::sync::Arc;

use axum::{Extension, extract::State, http::HeaderMap};
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    core::{
        Action, CoreSession, ListSessions, RevokeSession, SignIn, SignOut,
        SignedIn,
    },
    server::Req,
};

use super::{CoreState, internal};
use crate::access::{Caller, Peer, activity, fresh_ticket, now, sessions};

pub async fn sign_in(
    State(state): State<Arc<CoreState>>,
    peer: Option<Extension<Peer>>,
    Req { body: request, .. }: Req<SignIn>,
) -> Result<Json<SignedIn>, ApiError> {
    if let Some(Extension(Peer(address))) = peer
        && !state.access.allow_sign_in(address)
    {
        return Err(ApiError::new(
            ErrorCode::RateLimited,
            "Too many sign-ins from this address. Try again in a minute.",
        ));
    }
    let signed = sessions::sign_in(
        &state.db,
        &state.access.keys,
        &request.ticket,
        &request.device_name,
        now(),
    )
    .await?;
    Ok(Json(signed))
}

pub async fn list(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<ListSessions>,
) -> Result<Json<Vec<CoreSession>>, ApiError> {
    let only = if caller.is_owner() {
        None
    } else {
        caller.user_id()
    };
    let current = match &caller {
        Caller::Session { id, .. } => Some(id.as_str()),
        Caller::Local => None,
    };
    Ok(Json(
        sessions::list(&state.db, only, current, now())
            .await
            .map_err(internal)?,
    ))
}

pub async fn revoke(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    headers: HeaderMap,
    request: Req<RevokeSession>,
) -> Result<Empty, ApiError> {
    let id = request.params.id;
    let not_found =
        || ApiError::new(ErrorCode::NotFound, "There is no such session.");
    let session = sessions::get(&state.db, &id, None)
        .await
        .map_err(internal)?
        .ok_or_else(not_found)?;
    if caller.user_id() != Some(session.user.id.as_str()) {
        if !caller.is_owner() {
            return Err(not_found());
        }
        fresh_ticket(&state, &caller, &headers).await?;
    }
    end(&state, &caller, &id).await
}

pub async fn sign_out(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<SignOut>,
) -> Result<Empty, ApiError> {
    match &caller {
        Caller::Session { id, .. } => end(&state, &caller, id).await,
        Caller::Local => Err(ApiError::new(
            ErrorCode::InvalidInput,
            "The local CLI has no session to sign out of.",
        )),
    }
}

async fn end(
    state: &CoreState,
    caller: &Caller,
    id: &str,
) -> Result<Empty, ApiError> {
    if let Some(user) =
        sessions::revoke(&state.db, id).await.map_err(internal)?
    {
        state.access.changed(&user);
        activity::log(
            &state.db,
            caller,
            None,
            Action::SessionRevoked {
                session_id: id.into(),
            },
        )
        .await;
    }
    Ok(Empty)
}
