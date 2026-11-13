use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use super::endpoints;
use crate::state::{
    CacheBehaviour, CacheValueType, Organization, Project, ProjectV3,
    SearchResults, SearchResultsV3, TeamMember, User, Version,
};

endpoints! {
    /// `null` when Modrinth has no such project. The id may be a slug.
    GetCachedProject: Get "/cache/projects/{id}", CacheKey, CacheRead, Empty => Json<Option<Project>>;
    GetCachedProjects: Get "/cache/projects", (), CacheKeys, Empty => Json<Vec<Project>>;
    /// The project's versions, without changelogs.
    GetCachedProjectVersions: Get "/cache/projects/{id}/versions", CacheKey, CacheRead, Empty => Json<Option<Vec<Version>>>;
    GetCachedProjectV3: Get "/cache/v3/projects/{id}", CacheKey, CacheRead, Empty => Json<Option<ProjectV3>>;
    GetCachedProjectsV3: Get "/cache/v3/projects", (), CacheKeys, Empty => Json<Vec<ProjectV3>>;
    GetCachedVersion: Get "/cache/versions/{id}", CacheKey, CacheRead, Empty => Json<Option<Version>>;
    GetCachedVersions: Get "/cache/versions", (), CacheKeys, Empty => Json<Vec<Version>>;
    GetCachedUser: Get "/cache/users/{id}", CacheKey, CacheRead, Empty => Json<Option<User>>;
    GetCachedUsers: Get "/cache/users", (), CacheKeys, Empty => Json<Vec<User>>;
    GetCachedTeam: Get "/cache/teams/{id}", CacheKey, CacheRead, Empty => Json<Option<Vec<TeamMember>>>;
    GetCachedOrganization: Get "/cache/organizations/{id}", CacheKey, CacheRead, Empty => Json<Option<Organization>>;
    /// `id` is the search's query string.
    GetCachedSearchResults: Get "/cache/search", (), CacheSearch, Empty => Json<Option<SearchResults>>;
    GetCachedSearchResultsV3: Get "/cache/v3/search", (), CacheSearch, Empty => Json<Option<SearchResultsV3>>;
    /// Drops every cached entry of these types.
    PurgeCache: Post "/cache/purge", (), (), Json<Vec<CacheValueType>> => Empty;
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
pub struct CacheKey {
    pub id: String,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CacheRead {
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub cache_behaviour: Option<CacheBehaviour>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CacheKeys {
    /// A JSON array of ids.
    pub ids: String,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub cache_behaviour: Option<CacheBehaviour>,
}

#[derive(Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "camelCase")]
pub struct CacheSearch {
    pub id: String,
    #[cfg_attr(feature = "export-ts", ts(optional))]
    pub cache_behaviour: Option<CacheBehaviour>,
}
