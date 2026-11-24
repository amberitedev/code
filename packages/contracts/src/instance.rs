//! Instance enums both hosts' wire types name: the engine's instances and a Core's servers.

use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Copy, Deserialize, Serialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(rename_all = "lowercase")]
pub enum ModLoader {
    Vanilla,
    Forge,
    Fabric,
    Quilt,
    NeoForge,
    Paper,
}

impl ModLoader {
    pub fn as_str(&self) -> &'static str {
        match *self {
            Self::Vanilla => "vanilla",
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neoforge",
            Self::Paper => "paper",
        }
    }

    pub fn as_meta_str(&self) -> &'static str {
        match *self {
            Self::Vanilla => "vanilla",
            Self::Forge => "forge",
            Self::Fabric => "fabric",
            Self::Quilt => "quilt",
            Self::NeoForge => "neo",
            Self::Paper => "paper",
        }
    }

    pub fn from_string(val: &str) -> Self {
        match val {
            "vanilla" => Self::Vanilla,
            "forge" => Self::Forge,
            "fabric" => Self::Fabric,
            "quilt" => Self::Quilt,
            "neoforge" => Self::NeoForge,
            "paper" => Self::Paper,
            _ => Self::Vanilla,
        }
    }
}

/// Which Modrinth versions an instance's content updates to.
#[derive(Serialize, Deserialize, Debug, Clone, Copy, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
pub enum ReleaseChannel {
    Release,
    Beta,
    Alpha,
}

impl ReleaseChannel {
    pub fn key(self) -> &'static str {
        match self {
            Self::Release => "release",
            Self::Beta => "beta",
            Self::Alpha => "alpha",
        }
    }

    pub fn from_key(key: &str) -> Self {
        match key {
            "alpha" => Self::Alpha,
            "all" => Self::Alpha,
            "beta" => Self::Beta,
            _ => Self::Release,
        }
    }

    pub fn from_version_type(version_type: &str) -> Self {
        match version_type {
            "alpha" => Self::Alpha,
            "beta" => Self::Beta,
            _ => Self::Release,
        }
    }

    pub fn least_stable(self, other: Self) -> Self {
        if self.instability_rank() >= other.instability_rank() {
            self
        } else {
            other
        }
    }

    fn instability_rank(self) -> u8 {
        match self {
            Self::Release => 0,
            Self::Beta => 1,
            Self::Alpha => 2,
        }
    }

    pub fn version_type_fallbacks(self) -> Vec<Vec<&'static str>> {
        match self {
            Self::Release => {
                vec![vec!["release"], vec!["beta"], vec!["alpha"]]
            }
            Self::Beta => {
                vec![vec!["release", "beta"], vec!["alpha"]]
            }
            Self::Alpha => vec![vec!["release", "beta", "alpha"]],
        }
    }
}
