use super::*;
use crate::state::{CachedEntry, CachedSharedInstanceSummary};

#[tracing::instrument(skip(state))]
pub async fn get_cached_shared_instance_library(
    state: &Engine,
    user_id: &str,
) -> crate::Result<Vec<CachedSharedInstanceSummary>> {
    CachedEntry::get_shared_instance_library(
        user_id,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await
}

#[tracing::instrument(skip(state, instances))]
pub async fn cache_shared_instance_library(
    state: &Engine,
    user_id: &str,
    instances: Vec<CachedSharedInstanceSummary>,
) -> crate::Result<()> {
    CachedEntry::cache_shared_instance_library(user_id, instances, &state.pool)
        .await
}
