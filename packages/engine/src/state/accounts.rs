//! Reads of the App's account tables, which share the engine's database.

use crate::state::Sessions;
use sqlx::SqlitePool;
use uuid::Uuid;

/// The signed-in Modrinth user.
pub(crate) async fn active_modrinth_user_id(
    pool: &SqlitePool,
) -> crate::Result<Option<String>> {
    Ok(
        sqlx::query_scalar("SELECT id FROM modrinth_users WHERE active = TRUE")
            .fetch_optional(pool)
            .await?,
    )
}

/// The session of each Modrinth account saved for this engine's account service.
pub(crate) async fn modrinth_sessions(
    pool: &SqlitePool,
    sessions: &Sessions,
) -> crate::Result<Vec<String>> {
    Ok(sqlx::query_scalar(
        "SELECT users.session_id FROM modrinth_users users JOIN self_hosted_account_origins origins ON origins.user_id = users.id WHERE origins.origin = ? ORDER BY users.rowid",
    )
    .bind(crate::self_hosted::accounts::origin(sessions))
    .fetch_all(pool)
    .await?)
}

/// What a game log can show of a saved Minecraft account.
pub(crate) struct SavedMinecraftAccount {
    pub access_token: String,
    pub name: String,
    pub id: Uuid,
}

pub(crate) async fn minecraft_accounts(
    pool: &SqlitePool,
) -> crate::Result<Vec<SavedMinecraftAccount>> {
    let rows: Vec<(String, String, String)> = sqlx::query_as(
        "SELECT uuid, username, access_token FROM minecraft_users",
    )
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|(id, name, access_token)| SavedMinecraftAccount {
            access_token,
            name,
            id: Uuid::parse_str(&id).unwrap_or_default(),
        })
        .collect())
}
