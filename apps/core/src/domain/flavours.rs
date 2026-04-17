//! Domain flavours - Minecraft server variants.
//! Defines how to get download URLs for different flavours.

use thiserror::Error;
use url::Url;

/// Flavour error type.
#[derive(Error, Debug)]
pub enum FlavourError {
    #[error("Unsupported version: {0}")]
    UnsupportedVersion(String),
    #[error("Invalid URL")]
    InvalidUrl,
}

/// Flavour trait for server variants.
pub trait Flavour {
    fn get_download_url(&self, version: &str) -> Result<Url, FlavourError>;
    fn get_jar_filename(&self, version: &str) -> String;
    fn supports_version(&self, version: &str) -> bool;
}

/// Vanilla flavour.
pub struct Vanilla;

impl Flavour for Vanilla {
    fn get_download_url(&self, version: &str) -> Result<Url, FlavourError> {
        let hash = version_hash(version);
        let url = format!(
            "https://piston-data.mojang.com/v1/objects/{}/server.jar",
            hash
        );
        Url::parse(&url).map_err(|_| FlavourError::InvalidUrl)
    }

    fn get_jar_filename(&self, version: &str) -> String {
        format!("server-{}.jar", version)
    }

    fn supports_version(&self, _version: &str) -> bool {
        true
    }
}

/// Paper flavour.
pub struct Paper;

impl Flavour for Paper {
    fn get_download_url(&self, version: &str) -> Result<Url, FlavourError> {
        let url = format!(
            "https://api.papermc.io/v2/projects/paper/versions/{}/downloads/paper-{}-latest.jar",
            version, version
        );
        Url::parse(&url).map_err(|_| FlavourError::InvalidUrl)
    }

    fn get_jar_filename(&self, version: &str) -> String {
        format!("paper-{}-latest.jar", version)
    }

    fn supports_version(&self, version: &str) -> bool {
        version.starts_with("1.") && version.len() > 2
    }
}

/// Fabric flavour.
pub struct Fabric;

impl Flavour for Fabric {
    fn get_download_url(&self, version: &str) -> Result<Url, FlavourError> {
        let url = format!(
            "https://meta.fabricmc.net/v2/versions/game/{}/loader/latest/server/download",
            version
        );
        Url::parse(&url).map_err(|_| FlavourError::InvalidUrl)
    }

    fn get_jar_filename(&self, version: &str) -> String {
        format!("fabric-server-{}.jar", version)
    }

    fn supports_version(&self, version: &str) -> bool {
        version.starts_with("1.") && version.len() > 2
    }
}

fn version_hash(version: &str) -> String {
    match version {
        "1.20.1" => "304a1fb517310b9f304a1fb517310b9f304a1fb5".to_string(),
        "1.20.4" => "68d724734686c174681914f69d258c773420c611".to_string(),
        _ => "placeholder".to_string(),
    }
}
