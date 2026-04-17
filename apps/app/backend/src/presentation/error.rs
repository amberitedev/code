//! Error handling - Maps domain errors to HTTP responses.

use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use thiserror::Error;

/// API error type.
#[derive(Error, Debug)]
pub enum ApiError {
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Bad request: {0}")]
    BadRequest(String),
    #[error("Unauthorized: {0}")]
    Unauthorized(String),
    #[error("Forbidden: {0}")]
    Forbidden(String),
    #[error("Internal error")]
    Internal(String),
    #[error("Validation failed")]
    Validation { fields: Json<ValidationError> },
}

/// Validation error details.
#[derive(Serialize, Debug, Clone)]
pub struct ValidationError {
    pub errors: Vec<ValidationErrorDetail>,
}

#[derive(Serialize, Debug, Clone)]
pub struct ValidationErrorDetail {
    pub field: String,
    pub message: String,
}

impl ApiError {
    pub fn not_found(msg: &str) -> Self {
        ApiError::NotFound(msg.to_string())
    }

    pub fn bad_request(msg: &str) -> Self {
        ApiError::BadRequest(msg.to_string())
    }

    pub fn unauthorized(msg: &str) -> Self {
        ApiError::Unauthorized(msg.to_string())
    }

    pub fn forbidden(msg: &str) -> Self {
        ApiError::Forbidden(msg.to_string())
    }

    pub fn internal(msg: &str) -> Self {
        ApiError::Internal(msg.to_string())
    }

    pub fn validation(errors: Vec<ValidationErrorDetail>) -> Self {
        ApiError::Validation {
            fields: Json(ValidationError { errors }),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> axum::response::Response {
        let (status, body) = match &self {
            ApiError::NotFound(msg) => (axum::http::StatusCode::NOT_FOUND, msg),
            ApiError::BadRequest(msg) => (axum::http::StatusCode::BAD_REQUEST, msg),
            ApiError::Unauthorized(msg) => (axum::http::StatusCode::UNAUTHORIZED, msg),
            ApiError::Forbidden(msg) => (axum::http::StatusCode::FORBIDDEN, msg),
            ApiError::Internal(msg) => (axum::http::StatusCode::INTERNAL_SERVER_ERROR, msg),
            ApiError::Validation { fields } => {
                return (axum::http::StatusCode::UNPROCESSABLE_ENTITY, fields.clone())
                    .into_response()
            }
        };

        let body = Json(serde_json::json!({ "error": body.to_string() }));
        (status, body).into_response()
    }
}
