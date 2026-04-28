use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::application::{
    export_service::ExportError,
    instance_service::InstanceError,
    log_service::LogError,
    macro_service::MacroError,
    mod_service::ModError,
    modpack_service::ModpackError,
    stats_service::StatsError,
};

/// Unified API error type that maps to HTTP responses.
#[derive(Debug)]
pub enum ApiError {
    Unauthorized(String),
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),
    UnprocessableEntity(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            Self::Conflict(m) => (StatusCode::CONFLICT, m),
            Self::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
            Self::UnprocessableEntity(m) => (StatusCode::UNPROCESSABLE_ENTITY, m),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<InstanceError> for ApiError {
    fn from(e: InstanceError) -> Self {
        match e {
            InstanceError::NotFound(id) => Self::NotFound(format!("instance {id} not found")),
            InstanceError::AlreadyRunning => Self::Conflict("instance already running".into()),
            InstanceError::NotRunning => Self::Conflict("instance not running".into()),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<ModpackError> for ApiError {
    fn from(e: ModpackError) -> Self {
        match e {
            ModpackError::InstanceNotFound => Self::NotFound("instance not found".into()),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<MacroError> for ApiError {
    fn from(e: MacroError) -> Self {
        match e {
            MacroError::InstanceNotFound(id) => Self::NotFound(format!("instance {id} not found")),
            MacroError::FileNotFound(name) => Self::NotFound(format!("macro '{name}' not found")),
            MacroError::MacroNotFound(pid) => Self::NotFound(format!("macro pid {pid} not found")),
        }
    }
}

impl From<ModError> for ApiError {
    fn from(e: ModError) -> Self {
        match e {
            ModError::InstanceNotFound => Self::NotFound("instance not found".into()),
            ModError::ModNotFound => Self::NotFound("mod not found".into()),
            ModError::ClientOnly => Self::UnprocessableEntity("this mod is client-only".into()),
            ModError::NoModrinthId => Self::BadRequest("mod has no modrinth project id".into()),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<LogError> for ApiError {
    fn from(e: LogError) -> Self {
        match e {
            LogError::NotFound => Self::NotFound("not found".into()),
            LogError::InvalidPath => Self::BadRequest("invalid filename".into()),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<StatsError> for ApiError {
    fn from(e: StatsError) -> Self {
        match e {
            StatsError::NotFound => Self::NotFound("instance not found".into()),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<ExportError> for ApiError {
    fn from(e: ExportError) -> Self {
        match e {
            ExportError::InstanceNotFound => Self::NotFound("instance not found".into()),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

