use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::instance::ModLoader;

/// Deterministic, filesystem-safe identifier for a shared server installation.
///
/// Derived from the loader, game version, and loader version via
/// [`installation_key`], so two instances requesting the same build resolve to
/// the same installation directory under `{data_dir}/installations/{id}/`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstallationId(pub String);

impl std::fmt::Display for InstallationId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for InstallationId {
    type Err = std::convert::Infallible;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.to_string()))
    }
}

/// Readiness state of a shared installation's files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallationStatus {
    Installing,
    Ready,
    Failed,
}

impl std::fmt::Display for InstallationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Installing => "installing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for InstallationStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "installing" => Ok(Self::Installing),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown installation status: {s}")),
        }
    }
}

/// Persisted record for a shared server installation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerInstallationRecord {
    pub id: InstallationId,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub status: InstallationStatus,
    pub error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Build the deterministic installation key for a build triple.
///
/// Format: `{loader}-{game_version}-{loader_version|default}`, with every
/// character that is not alphanumeric, `.`, `-`, or `_` replaced by `_` so the
/// result is a safe single path segment on all platforms.
pub fn installation_key(
    loader: &ModLoader,
    game_version: &str,
    loader_version: Option<&str>,
) -> String {
    let lv = loader_version.unwrap_or("default");
    let raw = format!("{loader}-{game_version}-{lv}");
    sanitize_segment(&raw)
}

fn sanitize_segment(s: &str) -> String {
    s.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn installation_status_roundtrip() {
        for (status, s) in [
            (InstallationStatus::Installing, "installing"),
            (InstallationStatus::Ready, "ready"),
            (InstallationStatus::Failed, "failed"),
        ] {
            assert_eq!(status.to_string(), s);
            assert_eq!(s.parse::<InstallationStatus>().unwrap(), status);
        }
    }

    #[test]
    fn installation_key_is_deterministic_and_safe() {
        let a = installation_key(&ModLoader::Forge, "1.20.1", Some("47.2.0"));
        let b = installation_key(&ModLoader::Forge, "1.20.1", Some("47.2.0"));
        assert_eq!(a, b);
        assert_eq!(a, "forge-1.20.1-47.2.0");
    }

    #[test]
    fn installation_key_defaults_missing_loader_version() {
        let k = installation_key(&ModLoader::Vanilla, "1.21", None);
        assert_eq!(k, "vanilla-1.21-default");
    }

    #[test]
    fn installation_key_sanitizes_unsafe_chars() {
        let k = installation_key(&ModLoader::Fabric, "1.20/1", Some("a b:c"));
        assert!(!k.contains('/'));
        assert!(!k.contains(' '));
        assert!(!k.contains(':'));
    }
}
