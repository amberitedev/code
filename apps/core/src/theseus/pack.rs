//! Modpack handling - .mrpack installation

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use bytes::Bytes;
use async_zip::base::read::seek::ZipFileReader;
use std::io::Cursor;

use crate::theseus::error::{TheseusError, TheseusResult};
use crate::theseus::profile::{ModLoader, Profile};

/// Modrinth pack format (modrinth.index.json)
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackFormat {
    pub game: String,
    pub format_version: i32,
    pub version_id: String,
    pub name: String,
    pub summary: Option<String>,
    pub files: Vec<PackFile>,
    pub dependencies: HashMap<PackDependency, String>,
}

/// A file entry in the modpack
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PackFile {
    pub path: String,
    pub hashes: HashMap<PackFileHash, String>,
    pub env: Option<HashMap<EnvType, SideType>>,
    pub downloads: Vec<String>,
    pub file_size: u32,
}

/// Hash types for pack files
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PackFileHash {
    Sha1,
    Sha512,
}

impl From<String> for PackFileHash {
    fn from(s: String) -> Self {
        match s.as_str() {
            "sha1" => PackFileHash::Sha1,
            "sha512" => PackFileHash::Sha512,
            _ => PackFileHash::Sha1, // Default to sha1
        }
    }
}

/// Environment type (client or server)
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum EnvType {
    Client,
    Server,
}

/// Side support type
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum SideType {
    Required,
    Optional,
    Unsupported,
}

/// Pack dependency types
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PackDependency {
    #[serde(rename = "forge")]
    Forge,

    #[serde(rename = "neoforge")]
    #[serde(alias = "neo-forge")]
    NeoForge,

    #[serde(rename = "fabric-loader")]
    FabricLoader,

    #[serde(rename = "quilt-loader")]
    QuiltLoader,

    #[serde(rename = "minecraft")]
    Minecraft,
}

/// Extract metadata from an .mrpack file without installing
pub async fn extract_metadata(mrpack_bytes: &Bytes) -> TheseusResult<PackFormat> {
    let reader = Cursor::new(mrpack_bytes);
    let mut zip_reader = ZipFileReader::with_tokio(reader).await
        .map_err(|e| TheseusError::Zip(format!("Failed to read mrpack: {}", e)))?;
    
    // Find modrinth.index.json
    let manifest_idx = zip_reader.file().entries().iter().position(|f| {
        matches!(f.filename().as_str(), Ok("modrinth.index.json"))
    }).ok_or_else(|| {
        TheseusError::InvalidModpack("No modrinth.index.json found".to_string())
    })?;
    
    // Read manifest
    let mut manifest = String::new();
    let mut reader = zip_reader.reader_with_entry(manifest_idx).await?;
    reader.read_to_string_checked(&mut manifest).await?;
    
    let pack: PackFormat = serde_json::from_str(&manifest)?;
    
    if pack.game != "minecraft" {
        return Err(TheseusError::InvalidModpack(
            "Pack does not support Minecraft".to_string()
        ));
    }
    
    Ok(pack)
}

/// Install a modpack from .mrpack bytes to a target directory
/// 
/// This is the core function for server-side modpack installation.
/// It:
/// 1. Extracts modrinth.index.json
/// 2. Downloads all files from Modrinth
/// 3. Extracts overrides
/// 4. Returns the pack metadata
pub async fn install_mrpack(
    mrpack_bytes: Bytes,
    target_dir: &Path,
) -> TheseusResult<PackFormat> {
    // Create target directory
    tokio::fs::create_dir_all(target_dir).await?;
    
    // Extract metadata first
    let pack = extract_metadata(&mrpack_bytes).await?;
    
    let reader = Cursor::new(&mrpack_bytes);
    let mut zip_reader = ZipFileReader::with_tokio(reader).await
        .map_err(|e| TheseusError::Zip(format!("Failed to read mrpack: {}", e)))?;
    
    // Download all pack files from Modrinth
    for file in &pack.files {
        // Skip client-only files for server installation
        if let Some(env) = &file.env {
            if let Some(SideType::Unsupported) = env.get(&EnvType::Server) {
                tracing::info!("Skipping client-only file: {}", file.path);
                continue;
            }
        }
        
        // Download file from first available mirror
        if let Some(url) = file.downloads.first() {
            let response = reqwest::get(url).await?;
            let bytes = response.bytes().await?;
            
            // Verify hash if available
            if let Some(expected_hash) = file.hashes.get(&PackFileHash::Sha1) {
                let actual_hash = sha1_hash(&bytes);
                if actual_hash != *expected_hash {
                    return Err(TheseusError::HashMismatch {
                        expected: expected_hash.clone(),
                        actual: actual_hash,
                    });
                }
            }
            
            // Write file to target directory
            let file_path = target_dir.join(&file.path);
            if let Some(parent) = file_path.parent() {
                tokio::fs::create_dir_all(parent).await?;
            }
            tokio::fs::write(&file_path, &bytes).await?;
            
            tracing::info!("Downloaded {} ({} bytes)", file.path, bytes.len());
        }
    }
    
    // Extract overrides (config files, etc.)
    // Collect entry info first to avoid borrow checker issues
    let entries: Vec<_> = zip_reader
        .file()
        .entries()
        .iter()
        .enumerate()
        .map(|(idx, entry)| {
            let filename = entry.filename().as_str().unwrap_or_default().to_string();
            (idx, filename)
        })
        .filter(|(_, filename)| {
            (filename.starts_with("overrides/") || filename.starts_with("server-overrides/"))
                && !filename.ends_with('/')
        })
        .collect();
    
    for (index, filename) in entries {
        // Process overrides/ and server-overrides/ directories
        let relative_path = if filename.starts_with("overrides/") {
            filename.strip_prefix("overrides/").unwrap()
        } else {
            filename.strip_prefix("server-overrides/").unwrap()
        };
        
        let target_path = target_dir.join(relative_path);
        if let Some(parent) = target_path.parent() {
            tokio::fs::create_dir_all(parent).await?;
        }
        
        let mut file_bytes = Vec::new();
        let mut reader = zip_reader.reader_with_entry(index).await?;
        reader.read_to_end_checked(&mut file_bytes).await?;
        
        tokio::fs::write(&target_path, &file_bytes).await?;
        tracing::info!("Extracted override: {}", relative_path);
    }
    
    Ok(pack)
}

/// Parse loader from pack dependencies
pub fn get_loader_from_dependencies(deps: &HashMap<PackDependency, String>) -> ModLoader {
    if deps.contains_key(&PackDependency::FabricLoader) {
        ModLoader::Fabric
    } else if deps.contains_key(&PackDependency::Forge) {
        ModLoader::Forge
    } else if deps.contains_key(&PackDependency::NeoForge) {
        ModLoader::NeoForge
    } else if deps.contains_key(&PackDependency::QuiltLoader) {
        ModLoader::Quilt
    } else {
        ModLoader::Vanilla
    }
}

/// Get game version from pack dependencies
pub fn get_game_version(deps: &HashMap<PackDependency, String>) -> Option<String> {
    deps.get(&PackDependency::Minecraft).cloned()
}

/// Get loader version from pack dependencies
pub fn get_loader_version(deps: &HashMap<PackDependency, String>, loader: ModLoader) -> Option<String> {
    match loader {
        ModLoader::Fabric => deps.get(&PackDependency::FabricLoader).cloned(),
        ModLoader::Forge => deps.get(&PackDependency::Forge).cloned(),
        ModLoader::NeoForge => deps.get(&PackDependency::NeoForge).cloned(),
        ModLoader::Quilt => deps.get(&PackDependency::QuiltLoader).cloned(),
        ModLoader::Vanilla => None,
    }
}

/// Compute SHA1 hash of bytes
fn sha1_hash(bytes: &[u8]) -> String {
    use sha1::Digest;
    let mut hasher = sha1::Sha1::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}
