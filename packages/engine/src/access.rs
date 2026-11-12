//! Who may call an engine route. The engine asks its host through [`Access`]; the App allows its
//! one local user everything, a Core checks the caller's session and role.

use std::{marker::PhantomData, sync::Arc};

use axum::{
    extract::{FromRef, FromRequestParts, RawPathParams},
    http::request::Parts,
};
use contracts::{ApiError, ErrorCode};
pub use futures::future::BoxFuture;

pub enum Caller {
    Local,
    Session {
        id: String,
        user_id: String,
        owner: bool,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Permission {
    View,
    Edit,
    Owner,
}

pub trait Access: Send + Sync + 'static {
    fn caller<'a>(
        &'a self,
        request: &'a Parts,
    ) -> BoxFuture<'a, Result<Caller, ApiError>>;

    /// Returns the level granted, at least `permission`, so a route can show more to a caller
    /// with more. `instance` is `None` for a route over the whole engine.
    fn require<'a>(
        &'a self,
        caller: &'a Caller,
        instance: Option<&'a str>,
        permission: Permission,
    ) -> BoxFuture<'a, Result<Permission, ApiError>>;
}

/// The level a route needs, as a type: [`View`], [`Edit`] or [`Owner`].
pub trait Level: Send + Sync {
    const PERMISSION: Permission;
}

pub struct View;
pub struct Edit;
pub struct Owner;

impl Level for View {
    const PERMISSION: Permission = Permission::View;
}

impl Level for Edit {
    const PERMISSION: Permission = Permission::Edit;
}

impl Level for Owner {
    const PERMISSION: Permission = Permission::Owner;
}

/// A caller allowed `P` on the instance in the route's `{id}`.
pub struct InstanceAccess<P> {
    pub id: String,
    pub caller: Caller,
    pub granted: Permission,
    _level: PhantomData<P>,
}

impl<P: Level, S: Send + Sync> FromRequestParts<S> for InstanceAccess<P>
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
        let granted = access.require(&caller, Some(&id), P::PERMISSION).await?;
        Ok(Self {
            id,
            caller,
            granted,
            _level: PhantomData,
        })
    }
}

/// A caller allowed `P` over the whole engine, such as its groups or its loading bars.
pub struct EngineAccess<P> {
    pub caller: Caller,
    _level: PhantomData<P>,
}

impl<P: Level, S: Send + Sync> FromRequestParts<S> for EngineAccess<P>
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
        access.require(&caller, None, P::PERMISSION).await?;
        Ok(Self {
            caller,
            _level: PhantomData,
        })
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
