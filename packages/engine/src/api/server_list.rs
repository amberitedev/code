//! An instance's `servers.dat`: its codec, its entries and the plain edits an engine makes when
//! no host keeps the list in sync.

use crate::ErrorKind;
use crate::util::io;
use anyhow::{Context, bail};
use quartz_nbt::{NbtCompound, NbtList, NbtTag};
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;
use uuid::Uuid;

pub const SERVERS_FILE: &str = "servers.dat";

/// Where a modpack can ship its server list.
pub const MODPACK_SERVER_PATHS: [&str; 3] = [
    "servers.dat",
    "config/yosbr/servers.dat",
    "config/defaultoptions/servers.dat",
];

pub async fn read_servers(path: &Path) -> crate::Result<Vec<NbtCompound>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    servers_from_root(read_nbt_file(path).await?)
}

pub fn servers_from_bytes(bytes: Vec<u8>) -> crate::Result<Vec<NbtCompound>> {
    servers_from_root(nbt_from_bytes(bytes)?)
}

fn servers_from_root(root: NbtCompound) -> crate::Result<Vec<NbtCompound>> {
    let list =
        root.get::<_, &NbtList>("servers")
            .context(ErrorKind::InputError(
                "servers.dat does not contain a valid servers list".to_string(),
            ))?;
    list.iter()
        .map(|tag| match tag {
            NbtTag::Compound(compound) => Ok(compound.clone()),
            _ => Err(ErrorKind::InputError(
                "servers.dat contains an invalid server entry".to_string(),
            )
            .into()),
        })
        .collect()
}

pub async fn write_servers(
    path: &Path,
    servers: &[NbtCompound],
) -> crate::Result<()> {
    if let Some(parent) = path.parent() {
        crate::util::io::create_dir_all(parent).await?;
    }
    crate::util::io::write(path, servers_to_bytes(servers)?).await?;
    Ok(())
}

pub fn servers_to_bytes(servers: &[NbtCompound]) -> crate::Result<Vec<u8>> {
    let mut list = NbtList::new();
    for server in servers {
        list.push(server.clone());
    }
    let mut root = NbtCompound::new();
    root.insert("servers", list);
    nbt_to_bytes(&root)
}

pub fn server_data(
    name: String,
    address: String,
    accept_textures: Option<bool>,
) -> NbtCompound {
    let mut server = NbtCompound::new();
    server.insert("name", name);
    server.insert("ip", address);
    if let Some(accept_textures) = accept_textures {
        server.insert("acceptTextures", i8::from(accept_textures));
    }
    server.insert("hidden", 0_i8);
    server
}

pub fn update_server_data(
    server: &mut NbtCompound,
    name: String,
    address: String,
    accept_textures: Option<bool>,
) {
    server.insert("name", name);
    server.insert("ip", address);
    match accept_textures {
        Some(value) => server.insert("acceptTextures", i8::from(value)),
        None => {
            server.inner_mut().remove("acceptTextures");
        }
    }
}

pub fn server_hidden(server: &NbtCompound) -> bool {
    server.get::<_, i8>("hidden").unwrap_or(0) != 0
}

pub fn server_address(server: &NbtCompound) -> String {
    server.get::<_, &str>("ip").unwrap_or_default().to_string()
}

pub fn server_identity_address(server: &NbtCompound) -> String {
    server_address(server).trim().to_ascii_lowercase()
}

pub fn server_identity_name(server: &NbtCompound) -> String {
    server
        .get::<_, &str>("name")
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase()
}

pub async fn read_nbt_file(path: &Path) -> crate::Result<NbtCompound> {
    let bytes = io::read(path).await?;
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut Cursor::new(bytes),
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    Ok(root)
}

pub fn nbt_to_bytes(root: &NbtCompound) -> crate::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    quartz_nbt::io::write_nbt(
        &mut bytes,
        None,
        root,
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    Ok(bytes)
}

pub fn nbt_from_bytes(bytes: Vec<u8>) -> crate::Result<NbtCompound> {
    let (root, _) = quartz_nbt::io::read_nbt(
        &mut Cursor::new(bytes),
        quartz_nbt::io::Flavor::Uncompressed,
    )?;
    Ok(root)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum ServerSource {
    UserSynced,
    Modpack,
    LinkedServerProject,
    LocalDesynced,
}

impl ServerSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::UserSynced => "user_synced",
            Self::Modpack => "modpack",
            Self::LinkedServerProject => "linked_server_project",
            Self::LocalDesynced => "local_desynced",
        }
    }

    pub fn from_str(value: &str) -> Option<Self> {
        match value {
            "user_synced" => Some(Self::UserSynced),
            "modpack" => Some(Self::Modpack),
            "linked_server_project" => Some(Self::LinkedServerProject),
            "local_desynced" => Some(Self::LocalDesynced),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum DesyncServerMode {
    KeepInOtherInstances,
    RemoveFromOtherInstances,
}

#[derive(Clone, Debug)]
pub struct ServerRecord {
    pub id: String,
    pub source: ServerSource,
    pub data: NbtCompound,
}

impl ServerRecord {
    pub fn name(&self) -> String {
        self.data
            .get::<_, &str>("name")
            .unwrap_or_default()
            .to_string()
    }

    pub fn address(&self) -> String {
        self.data
            .get::<_, &str>("ip")
            .unwrap_or_default()
            .to_string()
    }

    pub fn icon(&self) -> Option<String> {
        self.data.get::<_, &str>("icon").ok().map(ToOwned::to_owned)
    }

    pub fn hidden(&self) -> bool {
        self.data.get::<_, i8>("hidden").unwrap_or(0) != 0
    }

    pub fn accept_textures(&self) -> Option<bool> {
        self.data
            .get::<_, i8>("acceptTextures")
            .ok()
            .map(|value| value != 0)
    }
}

/// The entries of the `servers.dat` at `path`, each under a fresh id.
pub async fn list_local(path: &Path) -> crate::Result<Vec<ServerRecord>> {
    Ok(read_servers(path)
        .await?
        .into_iter()
        .map(|data| ServerRecord {
            id: Uuid::new_v4().to_string(),
            source: ServerSource::LocalDesynced,
            data,
        })
        .collect())
}

/// Adds a server ahead of the hidden direct-connect entries.
pub async fn add_local(path: &Path, data: NbtCompound) -> crate::Result<()> {
    let mut servers = read_servers(path).await?;
    let insert_index = servers
        .iter()
        .position(server_hidden)
        .unwrap_or(servers.len());
    servers.insert(insert_index, data);
    write_servers(path, &servers).await
}

pub async fn update_local(
    path: &Path,
    index: usize,
    name: String,
    address: String,
    accept_textures: Option<bool>,
) -> crate::Result<()> {
    let mut servers = read_servers(path).await?;
    let server = servers
        .get_mut(index)
        .filter(|server| !server_hidden(server))
        .ok_or(ErrorKind::InputError(format!(
            "no editable server at index {index}"
        )))?;
    update_server_data(server, name, address, accept_textures);
    write_servers(path, &servers).await
}

pub async fn remove_local(path: &Path, index: usize) -> crate::Result<()> {
    let mut servers = read_servers(path).await?;
    if servers.get(index).is_none_or(server_hidden) {
        bail!(ErrorKind::InputError(format!(
            "no removable server at index {index}"
        )));
    }
    servers.remove(index);
    write_servers(path, &servers).await
}
