use super::*;
use crate::state::{
    CachedEntry, CachedSharedInstanceSummary, ModrinthCredentials,
};

#[tracing::instrument(skip(state))]
pub async fn get_cached_shared_instance_library(
    state: &Engine,
) -> crate::Result<Vec<CachedSharedInstanceSummary>> {
    let Some(user_id) =
        ModrinthCredentials::get_active_user_id(&state.pool).await?
    else {
        return Ok(Vec::new());
    };

    CachedEntry::get_shared_instance_library(
        &user_id,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await
}

#[tracing::instrument(skip(state, instances))]
pub async fn cache_shared_instance_library(
    state: &Engine,
    instances: Vec<CachedSharedInstanceSummary>,
) -> crate::Result<()> {
    let Some(user_id) =
        ModrinthCredentials::get_active_user_id(&state.pool).await?
    else {
        return Ok(());
    };

    CachedEntry::cache_shared_instance_library(&user_id, instances, &state.pool)
        .await
}
