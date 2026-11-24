use contracts::{Empty, Json};
use serde::{Deserialize, Serialize};

use crate::state::{
    CacheBehaviour, CacheValueType, Organization, Project, ProjectV3,
    SearchResults, SearchResultsV3, TeamMember, User, Version,
};

// The many-id reads are POSTs: a project's every version id overflows a request line.
contracts::endpoints! {
    group cache;

    /// `null` when Modrinth has no such project. The id may be a slug.
    GetCachedProject: Get "/cache/projects/{id}", Session, CacheKey, CacheRead, Empty => Json<Option<Project>>;
    GetCachedProjects: Post "/cache/projects", Session, (), (), Json<CacheKeys> => Json<Vec<Project>>;
    /// The project's versions, without changelogs.
    GetCachedProjectVersions: Get "/cache/projects/{id}/versions", Session, CacheKey, CacheRead, Empty => Json<Option<Vec<Version>>>;
    GetCachedProjectV3: Get "/cache/v3/projects/{id}", Session, CacheKey, CacheRead, Empty => Json<Option<ProjectV3>>;
    GetCachedProjectsV3: Post "/cache/v3/projects", Session, (), (), Json<CacheKeys> => Json<Vec<ProjectV3>>;
    GetCachedVersion: Get "/cache/versions/{id}", Session, CacheKey, CacheRead, Empty => Json<Option<Version>>;
    GetCachedVersions: Post "/cache/versions", Session, (), (), Json<CacheKeys> => Json<Vec<Version>>;
    GetCachedUser: Get "/cache/users/{id}", Session, CacheKey, CacheRead, Empty => Json<Option<User>>;
    GetCachedUsers: Post "/cache/users", Session, (), (), Json<CacheKeys> => Json<Vec<User>>;
    GetCachedTeam: Get "/cache/teams/{id}", Session, CacheKey, CacheRead, Empty => Json<Option<Vec<TeamMember>>>;
    GetCachedOrganization: Get "/cache/organizations/{id}", Session, CacheKey, CacheRead, Empty => Json<Option<Organization>>;
    /// `id` is the search's query string.
    GetCachedSearchResults: Get "/cache/search", Session, (), CacheSearch, Empty => Json<Option<SearchResults>>;
    GetCachedSearchResultsV3: Get "/cache/v3/search", Session, (), CacheSearch, Empty => Json<Option<SearchResultsV3>>;
    /// Drops every cached entry of these types.
    PurgeCache: Post "/cache/purge", Session, (), (), Json<Vec<CacheValueType>> => Empty;
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
    pub ids: Vec<String>,
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
