//! Auth API handlers - Login, registration, setup.

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};
use garde::Validate;
use std::sync::Arc;
use crate::application::registry::ServiceRegistry;
use crate::presentation::error::{ApiError, ValidationErrorDetail};

/// Login request.
#[derive(Deserialize, Validate)]
pub struct LoginRequest {
    #[garde(length(min = 3, max = 100))]
    pub username: String,
    #[garde(length(min = 8, max = 256))]
    pub password: String,
}

/// Login response.
#[derive(Serialize)]
pub struct LoginResponse {
    pub token: String,
    pub user_id: String,
}

/// Handle login.
pub async fn login(
    State(registry): State<Arc<ServiceRegistry>>,
    Json(req): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    req.validate(&()).map_err(|e| {
        ApiError::validation(vec![ValidationErrorDetail {
            field: "request".into(),
            message: e.to_string(),
        }])
    })?;

    let token = registry.auth_service.authenticate(&req.username, &req.password)
        .await
        .map_err(|_| ApiError::unauthorized("Invalid credentials"))?;

    Ok(Json(LoginResponse {
        token,
        user_id: "placeholder".into(),
    }))
}

/// Setup request.
#[derive(Deserialize, Validate)]
pub struct SetupRequest {
    #[garde(length(min = 16))]
    pub key: String,
    #[garde(length(min = 3, max = 100))]
    pub username: String,
    #[garde(length(min = 8, max = 256))]
    pub password: String,
}

/// Setup response.
#[derive(Serialize)]
pub struct SetupResponse {
    pub token: String,
}

/// Handle setup.
pub async fn setup(
    State(_registry): State<Arc<ServiceRegistry>>,
    Json(req): Json<SetupRequest>,
) -> Result<Json<SetupResponse>, ApiError> {
    req.validate(&()).map_err(|e| {
        ApiError::validation(vec![ValidationErrorDetail {
            field: "request".into(),
            message: e.to_string(),
        }])
    })?;

    Ok(Json(SetupResponse {
        token: "setup-token".into(),
    }))
}