use std::sync::Arc;

use crate::application::{
    social_models::{SocialError, SyncEvent, SyncSnapshot},
    state::AppState,
};

pub async fn list_snapshots(
    state: &Arc<AppState>,
    profile_id: &str,
) -> Result<Vec<SyncSnapshot>, SocialError> {
    Ok(sqlx::query_as("SELECT * FROM sync_snapshots WHERE profile_id = ? ORDER BY created_at DESC LIMIT 50")
		.bind(profile_id)
		.fetch_all(&state.pool)
		.await?)
}

pub async fn list_events(
    state: &Arc<AppState>,
    profile_id: &str,
) -> Result<Vec<SyncEvent>, SocialError> {
    Ok(sqlx::query_as("SELECT * FROM sync_events WHERE profile_id = ? ORDER BY created_at DESC LIMIT 50")
		.bind(profile_id)
		.fetch_all(&state.pool)
		.await?)
}
