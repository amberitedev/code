use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde_json::json;

use crate::{
    application::{
        export_service::ExportError, instance_service::InstanceError,
        log_service::LogError, macro_service::MacroError,
        mod_service::ModError, modpack_service::ModpackError,
        query_service::QueryServiceError, rcon_service::RconServiceError,
        social_models::SocialError, stats_service::StatsError,
        task_service::TaskError,
    },
    ports::instance_store::StoreError,
};

/// Unified API error type that maps to HTTP responses.
#[derive(Debug)]
pub enum ApiError {
    Unauthorized(String),
    Forbidden(String),
    NotFound(String),
    BadRequest(String),
    Conflict(String),
    Internal(String),
    UnprocessableEntity(String),
    TooManyRequests(String),
    ServiceUnavailable(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, msg) = match self {
            Self::Unauthorized(m) => (StatusCode::UNAUTHORIZED, m),
            Self::Forbidden(m) => (StatusCode::FORBIDDEN, m),
            Self::NotFound(m) => (StatusCode::NOT_FOUND, m),
            Self::BadRequest(m) => (StatusCode::BAD_REQUEST, m),
            Self::Conflict(m) => (StatusCode::CONFLICT, m),
            Self::Internal(m) => (StatusCode::INTERNAL_SERVER_ERROR, m),
            Self::UnprocessableEntity(m) => {
                (StatusCode::UNPROCESSABLE_ENTITY, m)
            }
            Self::TooManyRequests(m) => (StatusCode::TOO_MANY_REQUESTS, m),
            Self::ServiceUnavailable(m) => (StatusCode::SERVICE_UNAVAILABLE, m),
        };
        (status, Json(json!({ "error": msg }))).into_response()
    }
}

impl From<InstanceError> for ApiError {
    fn from(e: InstanceError) -> Self {
        match e {
            InstanceError::NotFound(id) => {
                Self::NotFound(format!("instance {id} not found"))
            }
            InstanceError::AlreadyRunning => {
                Self::Conflict("instance already running".into())
            }
            InstanceError::NotRunning => {
                Self::Conflict("instance not running".into())
            }
            InstanceError::ActorDead => Self::ServiceUnavailable(
                "instance actor is not responding".into(),
            ),
            InstanceError::NotReady(message) => {
                Self::Conflict(format!("instance is not ready: {message}"))
            }
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<ModpackError> for ApiError {
    fn from(e: ModpackError) -> Self {
        match e {
            ModpackError::InstanceNotFound => {
                Self::NotFound("instance not found".into())
            }
            ModpackError::MissingFile => {
                Self::BadRequest("version has no downloadable file".into())
            }
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<MacroError> for ApiError {
    fn from(e: MacroError) -> Self {
        match e {
            MacroError::InstanceNotFound(id) => {
                Self::NotFound(format!("instance {id} not found"))
            }
            MacroError::FileNotFound(name) => {
                Self::NotFound(format!("macro '{name}' not found"))
            }
            MacroError::MacroNotFound(pid) => {
                Self::NotFound(format!("macro pid {pid} not found"))
            }
            MacroError::InvalidName => {
                Self::BadRequest("invalid macro name".into())
            }
            MacroError::Disabled => {
                Self::ServiceUnavailable("macro runtime is disabled".into())
            }
        }
    }
}

impl From<ModError> for ApiError {
    fn from(e: ModError) -> Self {
        match e {
            ModError::InstanceNotFound => {
                Self::NotFound("instance not found".into())
            }
            ModError::ModNotFound => Self::NotFound("mod not found".into()),
            ModError::ClientOnly => {
                Self::UnprocessableEntity("this mod is client-only".into())
            }
            ModError::NoModrinthId => {
                Self::BadRequest("mod has no modrinth project id".into())
            }
            ModError::InvalidFilename => {
                Self::BadRequest("invalid filename".into())
            }
            ModError::HashMismatch { .. } => Self::BadRequest(e.to_string()),
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<LogError> for ApiError {
    fn from(e: LogError) -> Self {
        match e {
            LogError::NotFound => Self::NotFound("not found".into()),
            LogError::InvalidPath => {
                Self::BadRequest("invalid filename".into())
            }
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
            ExportError::InstanceNotFound => {
                Self::NotFound("instance not found".into())
            }
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<SocialError> for ApiError {
    fn from(e: SocialError) -> Self {
        match e {
            SocialError::NotFound => Self::NotFound("not found".into()),
            SocialError::Invalid(message)
                if message.starts_with("not authorized") =>
            {
                Self::Forbidden(message)
            }
            SocialError::Invalid(message) => Self::BadRequest(message),
            SocialError::Database(e) => Self::Internal(e.to_string()),
            SocialError::Io(e) => Self::Internal(e.to_string()),
            SocialError::Mrpack(e) => Self::BadRequest(e.to_string()),
        }
    }
}

impl From<RconServiceError> for ApiError {
    fn from(e: RconServiceError) -> Self {
        match e {
            RconServiceError::NotFound => {
                Self::NotFound("instance not found".into())
            }
            RconServiceError::NotEnabled => {
                Self::Conflict("rcon is not enabled for this instance".into())
            }
            RconServiceError::NotRunning => {
                Self::Conflict("instance is not running".into())
            }
            RconServiceError::Rcon(inner) => match inner {
                crate::infrastructure::minecraft::rcon::RconError::AuthFailed => {
                    Self::BadRequest(inner.to_string())
                }
                crate::infrastructure::minecraft::rcon::RconError::Timeout => {
                    Self::ServiceUnavailable(
                        "rcon connection timed out".into(),
                    )
                }
                _ => Self::ServiceUnavailable(inner.to_string()),
            },
            RconServiceError::Properties(inner) => {
                Self::Internal(inner.to_string())
            }
        }
    }
}

impl From<QueryServiceError> for ApiError {
    fn from(e: QueryServiceError) -> Self {
        match e {
            QueryServiceError::NotFound => {
                Self::NotFound("instance not found".into())
            }
            QueryServiceError::NotRunning => {
                Self::Conflict("instance is not running".into())
            }
            QueryServiceError::Ping(inner) => {
                Self::ServiceUnavailable(inner.to_string())
            }
        }
    }
}

impl From<TaskError> for ApiError {
    fn from(e: TaskError) -> Self {
        match e {
            TaskError::NotFound => Self::NotFound("task not found".into()),
            TaskError::InvalidType => {
                Self::BadRequest("invalid task type".into())
            }
            TaskError::InvalidCron => {
                Self::BadRequest("invalid cron expression".into())
            }
            e => Self::Internal(e.to_string()),
        }
    }
}

impl From<sqlx::Error> for ApiError {
    fn from(e: sqlx::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(id) => {
                Self::NotFound(format!("instance {id} not found"))
            }
            StoreError::Database(e) => Self::Internal(e.to_string()),
            StoreError::Parse(e) => Self::Internal(format!("parse error: {e}")),
        }
    }
}
