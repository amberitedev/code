//! JWT extraction for Core HTTP routes.

use std::sync::Arc;

use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{request::Parts, HeaderMap},
};

use crate::{
    application::{access_service, state::AppState},
    infrastructure::auth::jwks::Claims,
    presentation::error::ApiError,
};

/// Axum extractor that validates an owner JWT and yields its claims.
pub struct AuthUser(pub Claims);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        let token = bearer_token(&parts.headers).ok_or_else(|| {
            ApiError::Unauthorized("missing Authorization header".into())
        })?;

        let jwks_url = state.jwks_url().await.ok_or_else(|| {
            ApiError::Unauthorized("Core is not paired".into())
        })?;

        let audience = state.auth_audience().await;

        let claims = state
            .jwks_cache
            .validate(token, &jwks_url, &audience)
            .await
            .map_err(|e| ApiError::Unauthorized(e.to_string()))?;

        access_service::require_any_member(state, &claims.sub)
            .await
            .map_err(|e| ApiError::Forbidden(e.to_string()))?;

        Ok(Self(claims))
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let val = headers.get("authorization")?.to_str().ok()?;
    val.strip_prefix("Bearer ")
}

#[cfg(test)]
mod tests {
    use super::bearer_token;
    use axum::http::{header, HeaderMap, HeaderValue};

    fn with_auth(value: &str) -> HeaderMap {
        let mut m = HeaderMap::new();
        m.insert(header::AUTHORIZATION, HeaderValue::from_str(value).unwrap());
        m
    }

    #[test]
    fn valid_bearer_token() {
        let headers = with_auth("Bearer abc123");
        assert_eq!(bearer_token(&headers), Some("abc123"));
    }

    #[test]
    fn missing_authorization_header() {
        assert_eq!(bearer_token(&HeaderMap::new()), None);
    }

    #[test]
    fn wrong_scheme_returns_none() {
        let headers = with_auth("Basic abc123");
        assert_eq!(bearer_token(&headers), None);
    }

    #[test]
    fn bearer_with_empty_token() {
        // Spec: caller is responsible for rejecting empty tokens
        let headers = with_auth("Bearer ");
        assert_eq!(bearer_token(&headers), Some(""));
    }

    #[test]
    fn bearer_prefix_is_case_sensitive() {
        // HTTP convention: scheme names are case-insensitive, but our impl uses exact match
        let headers = with_auth("bearer abc123");
        assert_eq!(bearer_token(&headers), None);
    }
}
