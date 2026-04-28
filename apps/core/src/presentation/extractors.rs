use std::sync::Arc;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
};

use crate::{
    application::state::AppState,
    infrastructure::auth::jwks::Claims,
    presentation::error::ApiError,
};

/// Axum extractor that validates a Supabase JWT and yields its claims.
pub struct AuthUser(pub Claims);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(&parts.headers)
            .ok_or_else(|| ApiError::Unauthorized("missing Authorization header".into()))?;

        let jwks_url = state
            .jwks_url()
            .await
            .ok_or_else(|| ApiError::Unauthorized("Core not paired with Supabase".into()))?;

        let claims = state
            .jwks_cache
            .validate(token, &jwks_url)
            .await
            .map_err(|e| ApiError::Unauthorized(e.to_string()))?;

        Ok(Self(claims))
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let val = headers.get("authorization")?.to_str().ok()?;
    val.strip_prefix("Bearer ")
}
