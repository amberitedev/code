//! Modrinth's public API, without an account: versions to install, the projects behind files,
//! and newer versions. Copal reaches Modrinth through this client; it is also the
//! [`ContentMetadataProvider`] `resolve_content` picks versions with.

use std::{collections::HashMap, time::Duration};

use async_trait::async_trait;
use modrinth_content_management::{ContentMetadataProvider, VersionType};
use reqwest::StatusCode;
use serde::{Serialize, de::DeserializeOwned};

use crate::content::domain::channel::version_type_fallbacks;
use crate::content::model::modrinth::{
    Project, SidedVersion, Version, VersionEnvironment,
};

const API: &str = "https://api.modrinth.com/v2";
const API_V3: &str = "https://api.modrinth.com/v3";
/// The longest one request may take, start to end.
const TIMEOUT: Duration = Duration::from_secs(60);
/// Tries per request, the first included.
const ATTEMPTS: u32 = 3;

#[derive(Debug, thiserror::Error)]
pub enum ModrinthError {
    #[error("Modrinth: {url}: {source}")]
    Http {
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("Modrinth: {url} answered {status}")]
    Status { url: String, status: StatusCode },
    #[error("Modrinth: {url} sent an unreadable answer: {source}")]
    Json {
        url: String,
        #[source]
        source: serde_json::Error,
    },
}

impl ModrinthError {
    /// Whether trying again can help.
    fn transient(&self) -> bool {
        match self {
            Self::Http { .. } => true,
            Self::Status { status, .. } => {
                status.is_server_error()
                    || *status == StatusCode::TOO_MANY_REQUESTS
            }
            Self::Json { .. } => false,
        }
    }

    fn is_not_found(&self) -> bool {
        matches!(self, Self::Status { status, .. } if *status == StatusCode::NOT_FOUND)
    }
}

/// Modrinth's API through `http`, which carries the caller's user agent.
#[derive(Clone)]
pub struct Modrinth {
    http: reqwest::Client,
}

impl Modrinth {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    /// Send the request `build` makes, up to [`ATTEMPTS`] times while it fails in a way that can
    /// pass, and read its JSON answer. An answer that does not parse is not retried.
    async fn send<T: DeserializeOwned>(
        &self,
        url: &str,
        build: impl Fn(&reqwest::Client) -> reqwest::RequestBuilder,
    ) -> Result<T, ModrinthError> {
        let http = |source| ModrinthError::Http {
            url: url.to_owned(),
            source,
        };
        let mut tries = 0;
        let bytes = loop {
            tries += 1;
            let attempt = async {
                let response = build(&self.http)
                    .timeout(TIMEOUT)
                    .send()
                    .await
                    .map_err(http)?;
                let status = response.status();
                if !status.is_success() {
                    return Err(ModrinthError::Status {
                        url: url.to_owned(),
                        status,
                    });
                }
                response.bytes().await.map_err(http)
            };
            match attempt.await {
                Err(error) if tries < ATTEMPTS && error.transient() => {
                    tracing::debug!(%error, tries, "Modrinth request failed; trying again");
                    tokio::time::sleep(Duration::from_secs(u64::from(
                        tries * tries,
                    )))
                    .await;
                }
                result => break result?,
            }
        };
        serde_json::from_slice(&bytes).map_err(|source| ModrinthError::Json {
            url: url.to_owned(),
            source,
        })
    }

    async fn get<T: DeserializeOwned>(
        &self,
        url: String,
    ) -> Result<T, ModrinthError> {
        self.send(&url, |http| http.get(&url)).await
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &(impl Serialize + Sync),
    ) -> Result<T, ModrinthError> {
        let url = format!("{API}{path}");
        self.send(&url, |http| http.post(&url).json(body)).await
    }

    pub async fn version(&self, id: &str) -> Result<Version, ModrinthError> {
        self.get(format!("{API}/version/{id}")).await
    }

    /// A project's versions, newest first, that run on one of `loaders` and `game_version`.
    /// No loaders means any (packs, datapacks on Vanilla).
    pub async fn versions(
        &self,
        project: &str,
        loaders: &[&str],
        game_version: Option<&str>,
    ) -> Result<Vec<Version>, ModrinthError> {
        let mut query = Vec::new();
        if !loaders.is_empty() {
            query.push(format!("loaders={}", json_query(&loaders)));
        }
        if let Some(game_version) = game_version {
            query
                .push(format!("game_versions={}", json_query(&[game_version])));
        }
        self.get(format!(
            "{API}/project/{project}/version?{}",
            query.join("&")
        ))
        .await
    }

    pub async fn project(&self, id: &str) -> Result<Project, ModrinthError> {
        self.get(format!("{API}/project/{id}")).await
    }

    pub async fn projects(
        &self,
        ids: &[String],
    ) -> Result<Vec<Project>, ModrinthError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.get(format!("{API}/projects?ids={}", json_query(ids)))
            .await
    }

    pub async fn versions_by_id(
        &self,
        ids: &[String],
    ) -> Result<Vec<Version>, ModrinthError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.get(format!("{API}/versions?ids={}", json_query(ids)))
            .await
    }

    /// As [`Self::versions_by_id`], with where each version runs.
    pub async fn sided_versions(
        &self,
        ids: &[String],
    ) -> Result<Vec<SidedVersion>, ModrinthError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        self.get(format!("{API_V3}/versions?ids={}", json_query(ids)))
            .await
    }

    /// The version each file is, by SHA-1. Files Modrinth does not know are left out.
    pub async fn by_hashes(
        &self,
        sha1s: &[String],
    ) -> Result<HashMap<String, Version>, ModrinthError> {
        if sha1s.is_empty() {
            return Ok(HashMap::new());
        }
        self.post(
            "/version_files",
            &serde_json::json!({ "hashes": sha1s, "algorithm": "sha1" }),
        )
        .await
    }

    /// The newest version for each file, by SHA-1, that runs on `loaders` and `game_version`
    /// and is one of `version_types`.
    pub async fn updates(
        &self,
        sha1s: &[String],
        loaders: &[&str],
        game_version: &str,
        version_types: &[VersionType],
    ) -> Result<HashMap<String, Version>, ModrinthError> {
        if sha1s.is_empty() {
            return Ok(HashMap::new());
        }
        let mut body = serde_json::json!({
            "hashes": sha1s,
            "algorithm": "sha1",
            "game_versions": [game_version],
            "version_types": version_types,
        });
        if !loaders.is_empty() {
            body["loaders"] = serde_json::json!(loaders);
        }
        self.post("/version_files/update", &body).await
    }

    /// The newest version for each file, by SHA-1, on `channel`: as [`Self::updates`], asking
    /// for each step of [`version_type_fallbacks`] in turn the files the steps before found
    /// nothing for, as app-lib's `CachedFileUpdate` fetch does.
    pub async fn updates_on_channel(
        &self,
        sha1s: &[String],
        loaders: &[&str],
        game_version: &str,
        channel: VersionType,
    ) -> Result<HashMap<String, Version>, ModrinthError> {
        let mut found = HashMap::new();
        for version_types in version_type_fallbacks(channel) {
            let remaining: Vec<String> = sha1s
                .iter()
                .filter(|sha1| !found.contains_key(*sha1))
                .cloned()
                .collect();
            if remaining.is_empty() {
                break;
            }
            found.extend(
                self.updates(&remaining, loaders, game_version, version_types)
                    .await?,
            );
        }
        Ok(found)
    }

    /// The version each file is, by SHA-1, with where it runs. Files Modrinth does not know are
    /// left out.
    pub async fn sided_by_hashes(
        &self,
        sha1s: &[String],
    ) -> Result<
        HashMap<String, (Version, Option<VersionEnvironment>)>,
        ModrinthError,
    > {
        let versions = self.by_hashes(sha1s).await?;
        let ids: Vec<String> = versions
            .values()
            .map(|version| version.id.clone())
            .collect();
        let environments: HashMap<String, Option<VersionEnvironment>> = self
            .sided_versions(&ids)
            .await?
            .into_iter()
            .map(|version| (version.id, version.environment))
            .collect();
        Ok(versions
            .into_iter()
            .map(|(sha1, version)| {
                let environment =
                    environments.get(&version.id).copied().flatten();
                (sha1, (version, environment))
            })
            .collect())
    }
}

/// `values` as a JSON array, URL-encoded for a query string.
fn json_query(values: &(impl Serialize + ?Sized)) -> String {
    let json = serde_json::to_string(values).expect("strings serialize");
    url::form_urlencoded::byte_serialize(json.as_bytes()).collect()
}

/// Versions for `resolve_content`, straight from the API.
#[async_trait]
impl ContentMetadataProvider for Modrinth {
    async fn get_version(
        &mut self,
        version_id: &str,
    ) -> Result<
        Option<modrinth_content_management::Version>,
        modrinth_content_management::Error,
    > {
        match self.get(format!("{API}/version/{version_id}")).await {
            Ok(version) => Ok(Some(version)),
            Err(error) if error.is_not_found() => Ok(None),
            Err(error) => Err(provider_error(&error)),
        }
    }

    async fn get_project_versions(
        &mut self,
        project_id: &str,
    ) -> Result<
        Vec<modrinth_content_management::Version>,
        modrinth_content_management::Error,
    > {
        match self
            .get(format!("{API}/project/{project_id}/version"))
            .await
        {
            Ok(versions) => Ok(versions),
            Err(error) if error.is_not_found() => Ok(Vec::new()),
            Err(error) => Err(provider_error(&error)),
        }
    }
}

fn provider_error(error: &ModrinthError) -> modrinth_content_management::Error {
    modrinth_content_management::Error::Provider(error.to_string())
}
