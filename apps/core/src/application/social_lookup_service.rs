use std::sync::Arc;

use crate::application::{
    social_models::{CoreMember, SocialError, SyncProfile},
    state::AppState,
};

pub async fn ensure_metadata(state: &Arc<AppState>) -> Result<(), SocialError> {
    let now = now();
    sqlx::query("INSERT OR IGNORE INTO core_metadata (id, name, updated_at) VALUES (1, 'Copal', ?)")
		.bind(now)
		.execute(&state.pool)
		.await?;
    Ok(())
}

pub async fn get_member(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<CoreMember, SocialError> {
    sqlx::query_as("SELECT * FROM core_members WHERE user_id = ?")
        .bind(user_id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(SocialError::NotFound)
}

pub async fn get_sync_profile(
    state: &Arc<AppState>,
    id: &str,
) -> Result<SyncProfile, SocialError> {
    sqlx::query_as("SELECT * FROM sync_profiles WHERE id = ?")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(SocialError::NotFound)
}

pub fn validate_choice(
    value: &str,
    allowed: &[&str],
    field: &str,
) -> Result<(), SocialError> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(SocialError::Invalid(format!(
            "{field} must be one of {}",
            allowed.join(", ")
        )))
    }
}

pub fn now() -> String {
    chrono::Utc::now().to_rfc3339()
}
