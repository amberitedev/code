//! What Modrinth's public API answers, as far as either side reads it.

use chrono::{DateTime, Utc};
use modrinth_content_management::{Dependency, VersionType};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize)]
pub struct Version {
    pub id: String,
    pub project_id: String,
    pub version_number: String,
    #[serde(default)]
    pub version_type: VersionType,
    #[serde(default)]
    pub loaders: Vec<String>,
    #[serde(default)]
    pub game_versions: Vec<String>,
    #[serde(default)]
    pub files: Vec<VersionFile>,
    #[serde(default)]
    pub dependencies: Vec<Dependency>,
    pub date_published: Option<DateTime<Utc>>,
    /// Where the version runs. Only v3 of the API says (`Modrinth::versions_by_id`); every v2
    /// answer leaves it `None`.
    #[serde(default)]
    pub environment: Option<VersionEnvironment>,
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
    pub hashes: VersionHashes,
    pub size: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VersionHashes {
    pub sha1: String,
    pub sha512: Option<String>,
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

/// Where a version runs, as v3 of the API names it. Mirrors app-lib's `VersionEnvironment`
/// (`state/cache.rs`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionEnvironment {
    ClientAndServer,
    ClientOnly,
    ClientOnlyServerOptional,
    SingleplayerOnly,
    ServerOnly,
    ServerOnlyClientOptional,
    DedicatedServerOnly,
    ClientOrServer,
    ClientOrServerPrefersBoth,
    #[serde(other)]
    Unknown,
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
