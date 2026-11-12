use crate::api::Result;
use crate::engine_handle::EngineHandle;
use engine::prelude::*;

macro_rules! impl_cache_methods {
    ($(($variant:ident, $type:ty)),*) => {
        $(
            paste::paste! {
                #[tauri::command]
                pub async fn [<get_ $variant:snake>](handle: tauri::State<'_, EngineHandle>, id: &str, cache_behaviour: Option<CacheBehaviour>) -> Result<Option<$type>>
                {
                    let state = handle.get().await;
                    Ok(engine::cache::[<get_ $variant:snake>](&state, id, cache_behaviour).await?)
                }

                #[tauri::command]
                pub async fn [<get_ $variant:snake _many>](
                    handle: tauri::State<'_, EngineHandle>,
                    ids: Vec<String>,
                    cache_behaviour: Option<CacheBehaviour>,
                ) -> Result<Vec<$type>>
                {
                    let state = handle.get().await;
                    let ids = ids.iter().map(|x| &**x).collect::<Vec<&str>>();
                    let entries =
                        engine::cache::[<get_ $variant:snake _many>](&state, &*ids, cache_behaviour).await?;

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
    (User, User),
    (Team, Vec<TeamMember>),
    (Organization, Organization),
    (SearchResults, SearchResults),
    (SearchResultsV3, SearchResultsV3)
);

pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new("cache")
        .invoke_handler(tauri::generate_handler![
            get_project,
            get_project_many,
            get_project_v3,
            get_project_v3_many,
            get_version,
            get_version_many,
            get_user,
            get_user_many,
            get_team,
            get_team_many,
            get_organization,
            get_organization_many,
            get_search_results,
            get_search_results_many,
            get_search_results_v3,
            get_search_results_v3_many,
            purge_cache_types,
            get_project_versions,
        ])
        .build()
}

#[tauri::command]
pub async fn purge_cache_types(
    handle: tauri::State<'_, EngineHandle>,
    cache_types: Vec<CacheValueType>,
) -> Result<()> {
    let state = handle.get().await;
    Ok(engine::cache::purge_cache_types(&state, &cache_types).await?)
}

#[tauri::command]
pub async fn get_project_versions(
    handle: tauri::State<'_, EngineHandle>,
    project_id: &str,
    cache_behaviour: Option<CacheBehaviour>,
) -> Result<Option<Vec<Version>>> {
    let state = handle.get().await;
    Ok(
        engine::cache::get_project_versions(
            &state,
            project_id,
            cache_behaviour,
        )
        .await?,
    )
}
