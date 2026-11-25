//! Who may call an engine route. The engine asks its host through [`Access`]; the App allows its
//! one local user everything, a Core lets in its owner only.

use std::sync::Arc;

use axum::{
    extract::{FromRef, FromRequestParts, RawPathParams},
    http::request::Parts,
};
use contracts::{ApiError, ErrorCode};
pub use futures::future::BoxFuture;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Caller {
    /// The host's own user: the App's, or a Core's CLI on its local socket. Owns everything.
    Local,
    Session {
        id: String,
        user_id: String,
        /// The user owns the host.
        owner: bool,
    },
}

impl Caller {
    /// The signed-in user. `None` for the host's own local caller.
    pub fn user_id(&self) -> Option<&str> {
        match self {
            Self::Local => None,
            Self::Session { user_id, .. } => Some(user_id),
        }
    }

    pub fn is_owner(&self) -> bool {
        matches!(self, Self::Local | Self::Session { owner: true, .. })
    }
}

pub trait Access: Send + Sync + 'static {
    fn caller<'a>(
        &'a self,
        request: &'a Parts,
    ) -> BoxFuture<'a, Result<Caller, ApiError>>;

    /// Whether `caller` may use `instance`, or the whole engine when it is `None`.
    fn require<'a>(
        &'a self,
        caller: &'a Caller,
        instance: Option<&'a str>,
    ) -> BoxFuture<'a, Result<(), ApiError>>;
}

/// A caller allowed on the instance in the route's `{id}`.
pub struct InstanceAccess {
    pub id: String,
    pub caller: Caller,
}

impl<S: Send + Sync> FromRequestParts<S> for InstanceAccess
where
    Arc<dyn Access>: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, ApiError> {
        let params = RawPathParams::from_request_parts(parts, state)
            .await
            .map_err(|rejection| {
                ApiError::new(ErrorCode::InvalidInput, rejection.body_text())
            })?;
        let id = params
            .iter()
            .find_map(|(name, value)| (name == "id").then(|| value.to_owned()))
            .expect("an instance route has an {id} segment");
        let access = Arc::<dyn Access>::from_ref(state);
        let caller = access.caller(parts).await?;
        access.require(&caller, Some(&id)).await?;
        Ok(Self { id, caller })
    }
}

/// A caller allowed over the whole engine, such as its loading bars.
pub struct EngineAccess {
    pub caller: Caller,
}

impl<S: Send + Sync> FromRequestParts<S> for EngineAccess
where
    Arc<dyn Access>: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, ApiError> {
        let access = Arc::<dyn Access>::from_ref(state);
        let caller = access.caller(parts).await?;
        access.require(&caller, None).await?;
        Ok(Self { caller })
    }
}

impl<S: Send + Sync> FromRequestParts<S> for Caller
where
    Arc<dyn Access>: FromRef<S>,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, ApiError> {
        Arc::<dyn Access>::from_ref(state).caller(parts).await
    }
}
