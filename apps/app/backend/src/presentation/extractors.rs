//! Custom Axum extractors.

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use std::sync::Arc;
use crate::infrastructure::supabase_auth::{SupabaseJwtValidator, JwtError};

pub struct AuthExtractor {
    pub user_id: String,
    pub email: Option<String>,
}

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthExtractor
where
    S: Send + Sync,
{
    type Rejection = Response;

    async fn from_request_parts(
        parts: &mut Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let validator = state
            .downcast_ref::<(Arc<crate::application::registry::ServiceRegistry, Arc<SupabaseJwtValidator>)>()
            .ok_or_else(|| {
                (StatusCode::INTERNAL_SERVER_ERROR, "Auth not configured").into_response()
            })
            .map(|s| &s.1)?;

        let auth_header = parts
            .headers
            .get("Authorization")
            .and_then(|h| h.to_str().ok())
            .ok_or_else(|| {
                (StatusCode::UNAUTHORIZED, "Missing Authorization header").into_response()
            })?;

        let claims = validator.validate(auth_header).map_err(|e| {
            let msg = match e {
                JwtError::Expired => "Token expired",
                JwtError::InvalidFormat => "Invalid token format",
                JwtError::MissingToken => "Missing token",
                JwtError::ValidationFailed(_) => "Invalid token",
            };
            (StatusCode::UNAUTHORIZED, msg).into_response()
        })?;

        Ok(AuthExtractor {
            user_id: claims.sub,
            email: claims.email,
        })
    }
}
