pub mod minecraft_auth;
pub mod mr_auth;
pub mod oauth_utils;
pub mod routes;
pub mod self_hosted;
pub mod state;

use contracts::{ApiError, ErrorCode};
use serde::Deserialize;
use serde::de::value::{Error, StrDeserializer};

use state::MinecraftAuthenticationError;

/// The engine's code for `error`, or the Minecraft sign-in failure it carries.
pub fn error_code(error: &engine::Error) -> &'static str {
    match engine::error_code(error) {
        "internal_error" => error
            .downcast_ref::<MinecraftAuthenticationError>()
            .map_or("internal_error", MinecraftAuthenticationError::code),
        code => code,
    }
}

/// [`engine::routes::api_error`], coding Minecraft sign-in failures too.
pub fn api_error(error: engine::Error) -> ApiError {
    if error
        .downcast_ref::<MinecraftAuthenticationError>()
        .is_none()
    {
        return engine::routes::api_error(error);
    }
    tracing::warn!("{error:?}");
    let code = ErrorCode::deserialize(StrDeserializer::<Error>::new(
        error_code(&error),
    ))
    .unwrap_or(ErrorCode::Unknown);
    ApiError::new(code, format!("{error:#}"))
}
