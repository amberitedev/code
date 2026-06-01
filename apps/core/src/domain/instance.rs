use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Newtype wrapper around Uuid for type-safe instance IDs.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct InstanceId(pub Uuid);

impl InstanceId {
    pub fn new() -> Self {
        Self(Uuid::new_v4())
    }
}

impl Default for InstanceId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for InstanceId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::str::FromStr for InstanceId {
    type Err = uuid::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Uuid::parse_str(s).map(Self)
    }
}

/// Lifecycle state of an instance process.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceStatus {
    Offline,
    Starting,
    Running,
    Stopping,
    Crashed,
}

/// Installation/readiness state of the server files for an instance.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstanceInstallStatus {
    Installing,
    Ready,
    Failed,
}

impl std::fmt::Display for InstanceInstallStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Installing => "installing",
            Self::Ready => "ready",
            Self::Failed => "failed",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for InstanceInstallStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "installing" => Ok(Self::Installing),
            "ready" => Ok(Self::Ready),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown install status: {s}")),
        }
    }
}

impl std::fmt::Display for InstanceStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Offline => "offline",
            Self::Starting => "starting",
            Self::Running => "running",
            Self::Stopping => "stopping",
            Self::Crashed => "crashed",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for InstanceStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "offline" => Ok(Self::Offline),
            "starting" => Ok(Self::Starting),
            "running" => Ok(Self::Running),
            "stopping" => Ok(Self::Stopping),
            "crashed" => Ok(Self::Crashed),
            _ => Err(format!("unknown status: {s}")),
        }
    }
}

/// Minecraft server loader type.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Paper,
    Fabric,
    Forge,
    NeoForge,
    Quilt,
}

impl std::fmt::Display for ModLoader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            Self::Vanilla => "vanilla",
            Self::Paper => "paper",
            Self::Fabric => "fabric",
            Self::Forge => "forge",
            Self::NeoForge => "neoforge",
            Self::Quilt => "quilt",
        };
        write!(f, "{s}")
    }
}

impl std::str::FromStr for ModLoader {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "vanilla" => Ok(Self::Vanilla),
            "paper" => Ok(Self::Paper),
            "fabric" => Ok(Self::Fabric),
            "forge" => Ok(Self::Forge),
            "neoforge" => Ok(Self::NeoForge),
            "quilt" => Ok(Self::Quilt),
            _ => Err(format!("unknown loader: {s}")),
        }
    }
}

/// JVM memory allocation settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MemorySettings {
    pub min_mb: u32,
    pub max_mb: u32,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            min_mb: 512,
            max_mb: 4096,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instance_status_roundtrip() {
        let cases = [
            (InstanceStatus::Offline, "offline"),
            (InstanceStatus::Starting, "starting"),
            (InstanceStatus::Running, "running"),
            (InstanceStatus::Stopping, "stopping"),
            (InstanceStatus::Crashed, "crashed"),
        ];
        for (status, s) in cases {
            assert_eq!(status.to_string(), s);
            assert_eq!(s.parse::<InstanceStatus>().unwrap(), status);
        }
    }

    #[test]
    fn instance_install_status_roundtrip() {
        let cases = [
            (InstanceInstallStatus::Installing, "installing"),
            (InstanceInstallStatus::Ready, "ready"),
            (InstanceInstallStatus::Failed, "failed"),
        ];
        for (status, s) in cases {
            assert_eq!(status.to_string(), s);
            assert_eq!(s.parse::<InstanceInstallStatus>().unwrap(), status);
        }
    }

    #[test]
    fn instance_status_invalid_parse() {
        assert!("garbage".parse::<InstanceStatus>().is_err());
        // Status strings are lowercase — uppercase must fail
        assert!("Running".parse::<InstanceStatus>().is_err());
        assert!("".parse::<InstanceStatus>().is_err());
    }

    #[test]
    fn mod_loader_roundtrip() {
        let cases = [
            (ModLoader::Vanilla, "vanilla"),
            (ModLoader::Paper, "paper"),
            (ModLoader::Fabric, "fabric"),
            (ModLoader::Forge, "forge"),
            (ModLoader::NeoForge, "neoforge"),
            (ModLoader::Quilt, "quilt"),
        ];
        for (loader, s) in cases {
            assert_eq!(loader.to_string(), s);
            assert_eq!(s.parse::<ModLoader>().unwrap(), loader);
        }
    }

    #[test]
    fn mod_loader_invalid_parse() {
        assert!("Fabric".parse::<ModLoader>().is_err());
        assert!("spigot".parse::<ModLoader>().is_err());
    }

    #[test]
    fn memory_settings_default_is_sane() {
        let m = MemorySettings::default();
        assert_eq!(m.min_mb, 512);
        assert_eq!(m.max_mb, 4096);
        assert!(m.min_mb <= m.max_mb, "min_mb must not exceed max_mb");
    }

    #[test]
    fn instance_id_display_and_parse() {
        let id = InstanceId::new();
        let s = id.to_string();
        let parsed: InstanceId = s.parse().unwrap();
        assert_eq!(id, parsed);
    }

    #[test]
    fn instance_id_invalid_parse() {
        assert!("not-a-uuid".parse::<InstanceId>().is_err());
        assert!("".parse::<InstanceId>().is_err());
    }
}

/// Persisted record for a server instance.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceRecord {
    pub id: InstanceId,
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub port: u16,
    pub memory: MemorySettings,
    pub java_version: Option<i64>,
    /// Extra JVM flags inserted after the memory flags and before `-jar`/`@args`
    /// (e.g. Aikar's GC flags). Whitespace-separated, quotes respected.
    pub jvm_args: Option<String>,
    /// Extra program arguments appended after the server launch args
    /// (e.g. `--world myworld`). Whitespace-separated, quotes respected.
    pub server_args: Option<String>,
    pub install_status: InstanceInstallStatus,
    pub status: InstanceStatus,
    pub data_dir: String,
    /// Shared installation this instance launches from. `None` for legacy
    /// instances whose server files live directly in `data_dir`.
    pub installation_id: Option<String>,
    pub total_uptime_seconds: u64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
