//! Sessions: one per device per Core, made from a ticket (see `contracts::core::SignIn`). The
//! token goes in `Authorization: Bearer` on every other route. Core stores only its hash.

use std::sync::Arc;

use axum::{Extension, extract::State};
use contracts::{
    ApiError, Empty, ErrorCode, Json,
    core::{
        Action, CoreSession, ListSessions, RevokeSession, SignIn, SignOut,
        SignedIn,
    },
    server::Req,
};

use super::{CoreState, internal};
use crate::access::{
    Caller, Dangerous, Peer, activity, now, require_owner, sessions,
};

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
    require_owner(&caller)?;
    let current = match &caller {
        Caller::Session { id, .. } => Some(id.as_str()),
        Caller::Local => None,
    };
    Ok(Json(
        sessions::list(&state.db, current, now())
            .await
            .map_err(internal)?,
    ))
}

pub async fn revoke(
    Dangerous(caller): Dangerous,
    State(state): State<Arc<CoreState>>,
    request: Req<RevokeSession>,
) -> Result<Empty, ApiError> {
    let id = request.params.id;
    sessions::get(&state.db, &id, None)
        .await
        .map_err(internal)?
        .ok_or_else(|| {
            ApiError::new(ErrorCode::NotFound, "There is no such session.")
        })?;
    end(&state, &caller, &id).await
}

pub async fn sign_out(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<SignOut>,
) -> Result<Empty, ApiError> {
    require_owner(&caller)?;
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
