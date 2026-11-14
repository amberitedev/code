use crate::state::{
    CacheBehaviour, CacheValueType, CachedEntry, Organization, Project,
    ProjectV3, SearchResults, SearchResultsV3, TeamMember, User, Version,
};

macro_rules! impl_cache_methods {
    ($(($variant:ident, $type:ty)),*) => {
        $(
            paste::paste! {
                #[tracing::instrument(skip(state))]
                pub async fn [<get_ $variant:snake>](
                    state: &crate::Engine,
                    id: &str,
                    cache_behaviour: Option<CacheBehaviour>,
                ) -> crate::Result<Option<$type>>
                {
                    Ok(CachedEntry::[<get_ $variant:snake _many>](&[id], cache_behaviour, &state.pool, &state.api_semaphore, &state.sessions).await?.into_iter().next())
                }

                #[tracing::instrument(skip(state))]
                pub async fn [<get_ $variant:snake _many>](
                    state: &crate::Engine,
                    ids: &[&str],
                    cache_behaviour: Option<CacheBehaviour>,
                ) -> crate::Result<Vec<$type>>
                {
                    let entries =
                        CachedEntry::[<get_ $variant:snake _many>](ids, None, &state.pool, &state.api_semaphore, &state.sessions).await?;

                    Ok(entries)
                }
            }
        )*
    }
}

impl_cache_methods!(
    (Project, Project),
    (ProjectV3, ProjectV3),
    (Version, Version),
    (Team, Vec<TeamMember>),
    (Organization, Organization),
    (SearchResults, SearchResults),
    (SearchResultsV3, SearchResultsV3)
);

pub async fn purge_cache_types(
    state: &crate::Engine,
    cache_types: &[CacheValueType],
) -> crate::Result<()> {
    CachedEntry::purge_cache_types(cache_types, &state.pool).await?;

    Ok(())
}

/// Get versions for a project (without changelogs for fast loading).
/// Uses the cache system with the ProjectVersions cache type.
#[tracing::instrument(skip(state))]
pub async fn get_project_versions(
    state: &crate::Engine,
    project_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Option<Vec<Version>>> {
    CachedEntry::get_project_versions(
        project_id,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await
}

// Account summaries use the configured account service; public project authors
// remain in CachedEntry's public Modrinth user cache.
pub async fn get_user(
    state: &crate::Engine,
    id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Option<User>> {
    Ok(get_user_many(state, &[id], cache_behaviour)
        .await?
        .into_iter()
        .next())
}

pub async fn get_user_many(
    state: &crate::Engine,
    ids: &[&str],
    cache_behaviour: Option<CacheBehaviour>,
) -> crate::Result<Vec<User>> {
    if crate::self_hosted::accounts::enabled(&state.sessions) {
        return crate::self_hosted::accounts::users(
            ids,
            &state.api_semaphore,
            &state.sessions,
        )
        .await;
    }
    CachedEntry::get_user_many(
        ids,
        cache_behaviour,
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await
}
