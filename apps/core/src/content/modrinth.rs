//! What Core asks Modrinth's API: versions to install, the projects behind files, and newer
//! versions. Public endpoints, no account.

use std::{collections::HashMap, time::Duration};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

use crate::installs::download;

const API: &str = "https://api.modrinth.com/v2";
const TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Debug, thiserror::Error)]
pub enum ModrinthError {
    #[error(transparent)]
    Download(#[from] download::DownloadError),
    #[error("Modrinth: {0}")]
    Http(#[from] reqwest::Error),
}

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    pub date_published: Option<DateTime<Utc>>,
}

impl Version {
    /// The file a version is installed from.
    pub fn primary(&self) -> Option<&VersionFile> {
        self.files
            .iter()
            .find(|file| file.primary)
            .or_else(|| self.files.first())
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionFile {
    pub url: String,
    pub filename: String,
    #[serde(default)]
    pub primary: bool,
    pub hashes: Hashes,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Hashes {
    pub sha1: String,
    pub sha512: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Dependency {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub dependency_type: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Project {
    pub id: String,
    pub title: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub downloads: Option<u64>,
    pub followers: Option<u64>,
}

/// The Modrinth names of the loaders whose content runs on a server with `loader`.
pub fn loaders_for(loader: crate::installs::ModLoader) -> Vec<&'static str> {
    use crate::installs::ModLoader;
    match loader {
        ModLoader::Fabric => vec!["fabric"],
        // Quilt runs Fabric mods.
        ModLoader::Quilt => vec!["quilt", "fabric"],
        ModLoader::Forge => vec!["forge"],
        ModLoader::NeoForge => vec!["neoforge"],
        ModLoader::Paper => vec!["paper", "spigot", "bukkit"],
        ModLoader::Vanilla => vec![],
    }
}

/// Modrinth's API, from Core.
#[derive(Clone)]
pub struct Modrinth {
    http: reqwest::Client,
}

impl Modrinth {
    pub fn new(http: reqwest::Client) -> Self {
        Self { http }
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<T, ModrinthError> {
        Ok(download::json(&self.http, &format!("{API}{path}")).await?)
    }

    async fn post<T: DeserializeOwned>(
        &self,
        path: &str,
        body: &impl Serialize,
    ) -> Result<T, ModrinthError> {
        Ok(self
            .http
            .post(format!("{API}{path}"))
            .timeout(TIMEOUT)
            .json(body)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?)
    }

    pub async fn version(&self, id: &str) -> Result<Version, ModrinthError> {
        self.get(&format!("/version/{id}")).await
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
            query.push(format!(
                "loaders={}",
                urlencode(
                    &serde_json::to_string(loaders).expect("strings serialize")
                )
            ));
        }
        if let Some(game_version) = game_version {
            query.push(format!(
                "game_versions={}",
                urlencode(
                    &serde_json::to_string(&[game_version])
                        .expect("strings serialize")
                )
            ));
        }
        self.get(&format!("/project/{project}/version?{}", query.join("&")))
            .await
    }

    pub async fn project(&self, id: &str) -> Result<Project, ModrinthError> {
        self.get(&format!("/project/{id}")).await
    }

    pub async fn projects(
        &self,
        ids: &[String],
    ) -> Result<Vec<Project>, ModrinthError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = serde_json::to_string(ids).expect("strings serialize");
        self.get(&format!("/projects?ids={}", urlencode(&ids)))
            .await
    }

    pub async fn versions_by_id(
        &self,
        ids: &[String],
    ) -> Result<Vec<Version>, ModrinthError> {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let ids = serde_json::to_string(ids).expect("strings serialize");
        self.get(&format!("/versions?ids={}", urlencode(&ids)))
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

    /// The newest version for each file, by SHA-1, that runs on `loaders` and `game_version`.
    pub async fn updates(
        &self,
        sha1s: &[String],
        loaders: &[&str],
        game_version: &str,
    ) -> Result<HashMap<String, Version>, ModrinthError> {
        if sha1s.is_empty() {
            return Ok(HashMap::new());
        }
        let mut body = serde_json::json!({
            "hashes": sha1s,
            "algorithm": "sha1",
            "game_versions": [game_version],
        });
        if !loaders.is_empty() {
            body["loaders"] = serde_json::json!(loaders);
        }
        self.post("/version_files/update", &body).await
    }
}

fn urlencode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// The project and version of a file on Modrinth's CDN, from its URL:
/// `https://cdn.modrinth.com/data/<project>/versions/<version>/<file>`.
pub fn ids_from_cdn(url: &str) -> Option<(String, String)> {
    let rest = url.strip_prefix("https://cdn.modrinth.com/data/")?;
    let mut parts = rest.split('/');
    let project = parts.next()?;
    (parts.next()? == "versions").then_some(())?;
    let version = parts.next()?;
    parts.next()?;
    Some((project.to_owned(), version.to_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cdn_urls_name_their_project_and_version() {
        assert_eq!(
            ids_from_cdn(
                "https://cdn.modrinth.com/data/AANobbMI/versions/u6dRKJwZ/sodium.jar"
            ),
            Some(("AANobbMI".into(), "u6dRKJwZ".into()))
        );
        assert_eq!(ids_from_cdn("https://github.com/a/b/releases/x.jar"), None);
        assert_eq!(
            ids_from_cdn("https://cdn.modrinth.com/data/AANobbMI/icon.png"),
            None
        );
    }
}
