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

/// The signed-in Modrinth user.
pub async fn active_modrinth_user_id(
    pool: &sqlx::SqlitePool,
) -> engine::Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM modrinth_users WHERE active = TRUE")
            .fetch_optional(pool)
            .await?,
    )
}

/// The saved accounts that game logs and support details hide: every Minecraft account, and the
/// Modrinth sessions saved for this engine's account service.
pub async fn saved_secrets(
    pool: &sqlx::SqlitePool,
    sessions: &engine::Sessions,
) -> engine::Result<engine::SavedSecrets> {
    let modrinth_sessions = sqlx::query_scalar(
        "SELECT users.session_id FROM modrinth_users users JOIN self_hosted_account_origins origins ON origins.user_id = users.id WHERE origins.origin = ? ORDER BY users.rowid",
    )
    .bind(engine::self_hosted::accounts::origin(sessions))
    .fetch_all(pool)
    .await?;
    let minecraft: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT uuid, username, access_token FROM minecraft_users",
    )
    .fetch_all(pool)
    .await?;
    Ok(engine::SavedSecrets {
        minecraft: minecraft
            .into_iter()
            .map(|(id, name, access_token)| engine::SavedMinecraftAccount {
                access_token,
                name,
                id: uuid::Uuid::parse_str(&id).unwrap_or_default(),
            })
            .collect(),
        modrinth_sessions,
    })
}
