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

const NO_AUTH_USER_ID: &str = "local-noauth-owner";

/// Axum extractor that validates an owner JWT and yields its claims.
pub struct AuthUser(pub Claims);

#[async_trait]
impl FromRequestParts<Arc<AppState>> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &Arc<AppState>,
    ) -> Result<Self, Self::Rejection> {
        if state.config.no_auth {
            return Ok(Self(no_auth_claims()));
        }

        let token = bearer_token(&parts.headers).ok_or_else(|| {
            ApiError::Unauthorized("missing Authorization header".into())
        })?;

        let mut claims = if state.config.dev_mode {
            if let Some(claims) = dev_claims(token)? {
                claims
            } else {
                validate_jwt(state, token).await?
            }
        } else {
            validate_jwt(state, token).await?
        };
        claims.sub = auth_user_id(&claims.sub).to_string();

        access_service::require_any_member(state, &claims.sub)
            .await
            .map_err(|e| ApiError::Forbidden(e.to_string()))?;

        Ok(Self(claims))
    }
}

fn no_auth_claims() -> Claims {
    Claims {
        sub: NO_AUTH_USER_ID.to_string(),
        aud: "no-auth".to_string(),
        role: Some("owner".to_string()),
        exp: (chrono::Utc::now().timestamp() + 3600) as u64,
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let val = headers.get("authorization")?.to_str().ok()?;
    val.strip_prefix("Bearer ")
}

fn auth_user_id(subject: &str) -> &str {
    match subject.split_once('|') {
        Some((user_id, session_id))
            if !user_id.is_empty() && !session_id.is_empty() =>
        {
            user_id
        }
        _ => subject,
    }
}

async fn validate_jwt(
    state: &Arc<AppState>,
    token: &str,
) -> Result<Claims, ApiError> {
    let jwks_url = state
        .jwks_url()
        .await
        .ok_or_else(|| ApiError::Unauthorized("Core is not paired".into()))?;

    let audience = state.auth_audience().await.ok_or_else(|| {
        ApiError::Unauthorized("Core auth audience is not configured".into())
    })?;

    state
        .jwks_cache
        .validate(token, &jwks_url, &audience)
        .await
        .map_err(|e| ApiError::Unauthorized(e.to_string()))
}

fn dev_claims(token: &str) -> Result<Option<Claims>, ApiError> {
    let Some(user_id) = token.strip_prefix("dev:") else {
        return Ok(None);
    };
    if user_id.is_empty() || user_id.chars().any(char::is_control) {
        return Err(ApiError::Unauthorized("invalid dev token".into()));
    }
    Ok(Some(Claims {
        sub: user_id.to_string(),
        aud: "dev".to_string(),
        role: Some("dev".to_string()),
        exp: (chrono::Utc::now().timestamp() + 3600) as u64,
    }))
}

#[cfg(test)]
mod tests {
    use super::{auth_user_id, bearer_token};
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

    #[test]
    fn convex_auth_subject_uses_user_id() {
        assert_eq!(
            auth_user_id("users:abc123|sessions:def456"),
            "users:abc123"
        );
    }

    #[test]
    fn bare_subject_is_unchanged() {
        assert_eq!(auth_user_id("dev-owner"), "dev-owner");
    }

    #[test]
    fn malformed_subject_is_unchanged() {
        assert_eq!(auth_user_id("users:abc123|"), "users:abc123|");
        assert_eq!(auth_user_id("|sessions:def456"), "|sessions:def456");
    }
}
