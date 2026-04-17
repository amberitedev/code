//! Custom Axum extractors.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use std::sync::Arc;
use crate::application::auth_service::AuthService;
use crate::application::instance_service::InstanceService;

/// Auth extractor.
pub struct AuthExtractor(pub Arc<AuthService>);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthExtractor
where
    S: Send + Sync,
{
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}

/// Instance service extractor.
pub struct InstanceServiceExtractor(pub Arc<InstanceService>);

#[axum::async_trait]
impl<S> FromRequestParts<S> for InstanceServiceExtractor
where
    S: Send + Sync,
{
    type Rejection = axum::response::Response;

    async fn from_request_parts(
        _parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        unimplemented!()
    }
}
