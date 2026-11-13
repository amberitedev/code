pub use engine::server_list::{DesyncServerMode, ServerRecord, ServerSource};
use quartz_nbt::NbtCompound;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
pub struct SyncedServer {
    pub id: String,
    pub name: String,
    pub address: String,
    #[cfg_attr(test, ts(optional = nullable))]
    pub accept_textures: Option<bool>,
}

#[derive(Clone)]
pub(super) struct CanonicalServer {
    pub id: String,
    pub data: NbtCompound,
}

#[derive(Clone)]
pub(super) struct LocalServer {
    pub id: String,
    pub source: ServerSource,
    pub excluded_synced_server_id: Option<String>,
    pub data: NbtCompound,
    pub position: i64,
}

#[derive(Clone)]
pub(super) struct ProjectionEntry {
    pub id: String,
    pub owner: ProjectionOwner,
    pub data: NbtCompound,
    pub position: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum ProjectionOwner {
    Synced,
    Instance,
}

impl ProjectionOwner {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Synced => "synced",
            Self::Instance => "instance",
        }
    }

    pub(super) fn from_str(value: &str) -> Option<Self> {
        match value {
            "synced" => Some(Self::Synced),
            "instance" => Some(Self::Instance),
            _ => None,
        }
    }
}
