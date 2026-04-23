//! Profile management - Minecraft instance profiles
//! Simplified from Theseus's Profile system

use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use uuid::Uuid;
use std::path::PathBuf;

/// Represents a Minecraft instance/profile
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Profile {
    pub path: String,
    pub install_stage: ProfileInstallStage,
    
    pub name: String,
    pub icon_path: Option<String>,
    
    pub game_version: String,
    pub protocol_version: Option<u32>,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
    pub last_played: Option<DateTime<Utc>>,
    
    // Server-specific settings
    pub java_path: Option<String>,
    pub extra_launch_args: Option<Vec<String>>,
    pub custom_env_vars: Option<Vec<(String, String)>>,
    
    pub memory: Option<MemorySettings>,
    
    // Link to Modrinth modpack
    pub linked_data: Option<LinkedData>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileInstallStage {
    /// Profile is installed
    Installed,
    /// Profile's minecraft game is still installing
    MinecraftInstalling,
    /// Pack is installed, but Minecraft installation has not begun
    PackInstalled,
    /// Profile created for pack, but the pack hasn't been fully installed yet
    PackInstalling,
    /// Profile is not installed
    NotInstalled,
}

impl ProfileInstallStage {
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Installed => "installed",
            Self::MinecraftInstalling => "minecraft_installing",
            Self::PackInstalled => "pack_installed",
            Self::PackInstalling => "pack_installing",
            Self::NotInstalled => "not_installed",
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct LinkedData {
    pub project_id: String,
    pub version_id: String,
    pub locked: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Forge,
    Fabric,
    Quilt,
    NeoForge,
}

impl ModLoader {
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Vanilla => "vanilla",
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neoforge",
        }
    }
    
    pub fn as_meta_str(&self) -> &'static str {
        match *self {
            Self::Vanilla => "vanilla",
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neo",
        }
    }
    
    pub fn from_string(val: &str) -> Self {
        match val {
            "vanilla" => Self::Vanilla,
            "forge" => Self::Forge,
            "fabric" => Self::Fabric,
            "quilt" => Self::Quilt,
            "neoforge" => Self::NeoForge,
            _ => Self::Vanilla,
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
pub struct MemorySettings {
    pub minimum: u32,
    pub maximum: u32,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            minimum: 512,
            maximum: 4096,
        }
    }
}

impl Profile {
    /// Create a new profile from modpack metadata
    pub fn new_from_pack(
        path: String,
        name: String,
        game_version: String,
        loader: ModLoader,
        loader_version: Option<String>,
        linked_data: Option<LinkedData>,
    ) -> Self {
        let now = Utc::now();
        Self {
            path,
            install_stage: ProfileInstallStage::NotInstalled,
            name,
            icon_path: None,
            game_version,
            protocol_version: None,
            loader,
            loader_version,
            created: now,
            modified: now,
            last_played: None,
            java_path: None,
            extra_launch_args: None,
            custom_env_vars: None,
            memory: Some(MemorySettings::default()),
            linked_data,
        }
    }
    
    /// Get the full path to this profile's directory
    pub fn get_full_path(&self, profiles_dir: &PathBuf) -> PathBuf {
        profiles_dir.join(&self.path)
    }
    
    /// Check if the profile is installed
    pub fn is_installed(&self) -> bool {
        matches!(self.install_stage, ProfileInstallStage::Installed)
    }
    
    /// Get memory settings with defaults
    pub fn memory(&self) -> MemorySettings {
        self.memory.unwrap_or_default()
    }
}

/// Directory structure for Theseus data
#[derive(Clone, Debug)]
pub struct DirectoryInfo {
    pub base_dir: PathBuf,
}

impl DirectoryInfo {
    pub fn new(base_dir: PathBuf) -> Self {
        Self { base_dir }
    }
    
    pub fn profiles_dir(&self) -> PathBuf {
        self.base_dir.join("profiles")
    }
    
    pub fn libraries_dir(&self) -> PathBuf {
        self.base_dir.join("libraries")
    }
    
    pub fn assets_dir(&self) -> PathBuf {
        self.base_dir.join("assets")
    }
    
    pub fn versions_dir(&self) -> PathBuf {
        self.base_dir.join("versions")
    }
    
    pub fn version_dir(&self, version_id: &str) -> PathBuf {
        self.versions_dir().join(version_id)
    }
}
