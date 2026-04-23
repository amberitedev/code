//! Modpack API handlers - Upload, download, and manage modpacks.
//! 
//! These endpoints handle:
//! - POST /instances - Upload a .mrpack file and install it
//! - GET /instances/:id/modpack - Download the stored .mrpack file
//! - Other modpack management endpoints

use axum::body::Bytes;
use axum::extract::{Multipart, Path, State};
use axum::response::IntoResponse;
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;
use std::path::PathBuf;
use std::sync::Arc;

use crate::application::registry::ServiceRegistry;
use crate::presentation::error::ApiError;
use crate::theseus::pack::{install_mrpack, extract_metadata, PackFormat};
use crate::theseus::profile::{LinkedData, Profile};
use crate::theseus::state::State as TheseusState;

/// Upload and install a modpack from .mrpack file
/// 
/// This endpoint:
/// 1. Receives a multipart upload with the .mrpack file
/// 2. Extracts metadata from modrinth.index.json
/// 3. Installs the modpack (downloads mods from Modrinth)
/// 4. Stores the original .mrpack for serving to friends later
/// 5. Returns instance metadata
pub async fn upload_modpack(
    State(_registry): State<Arc<ServiceRegistry>>,
    mut multipart: Multipart,
) -> Result<Json<ModpackUploadResponse>, ApiError> {
    let theseus_state = TheseusState::get().await
        .map_err(|e| ApiError::internal(&format!("Theseus state error: {}", e)))?;
    
    // Get the .mrpack file from multipart
    let mut mrpack_bytes: Option<Bytes> = None;
    
    while let Some(field) = multipart.next_field().await
        .map_err(|e| ApiError::bad_request(&format!("Upload error: {}", e)))? 
    {
        let name = field.name().unwrap_or("").to_string();
        if name == "mrpack" || name == "file" {
            let data = field.bytes().await
                .map_err(|e| ApiError::bad_request(&format!("Read error: {}", e)))?;
            mrpack_bytes = Some(data);
        }
    }
    
    let mrpack_bytes = mrpack_bytes
        .ok_or_else(|| ApiError::bad_request("No .mrpack file provided"))?;
    
    // Extract metadata first (before installation)
    let metadata = extract_metadata(&mrpack_bytes).await
        .map_err(|e| ApiError::bad_request(&format!("Invalid modpack: {}", e)))?;
    
    // Generate a unique instance ID
    let instance_id = Uuid::new_v4();
    let profile_path = format!("instance_{}", instance_id);
    
    // Create installation directory
    let install_dir = theseus_state.directories.profiles_dir()
        .join(&profile_path);
    
    // Install the modpack
    install_mrpack(mrpack_bytes.clone(), &install_dir).await
        .map_err(|e| ApiError::internal(&format!("Installation failed: {}", e)))?;
    
    // Store the original .mrpack file
    let modpack_path = install_dir.join("modpack.mrpack");
    tokio::fs::write(&modpack_path, &mrpack_bytes).await
        .map_err(|e| ApiError::internal(&format!("Failed to store modpack: {}", e)))?;
    
    // Create profile from metadata
    let loader = crate::theseus::pack::get_loader_from_dependencies(&metadata.dependencies);
    let loader_version = crate::theseus::pack::get_loader_version(&metadata.dependencies, loader.clone());
    let game_version = crate::theseus::pack::get_game_version(&metadata.dependencies)
        .unwrap_or_else(|| "1.20.1".to_string());
    
    let linked_data = Some(LinkedData {
        project_id: format!("modpack_{}", instance_id),
        version_id: metadata.version_id.clone(),
        locked: true,
    });
    
    let profile = Profile::new_from_pack(
        profile_path,
        metadata.name.clone(),
        game_version.clone(),
        loader,
        loader_version,
        linked_data,
    );
    
    // TODO: Save profile to database
    // For now, just return the response
    
    Ok(Json(ModpackUploadResponse {
        instance_id: instance_id.to_string(),
        name: profile.name,
        game_version,
        loader: profile.loader.as_str().to_string(),
        loader_version: profile.loader_version,
        summary: metadata.summary,
    }))
}

/// Download the original .mrpack file for an instance
/// 
/// Friends use this endpoint to download the modpack so they can
/// install it locally in their Minecraft client.
pub async fn download_modpack(
    Path(instance_id): Path<String>,
    State(_registry): State<Arc<ServiceRegistry>>,
) -> Result<impl IntoResponse, ApiError> {
    let theseus_state = TheseusState::get().await
        .map_err(|e| ApiError::internal(&format!("Theseus state error: {}", e)))?;
    
    // Construct the modpack path
    let profile_path = format!("instance_{}", instance_id);
    let modpack_path = theseus_state.directories.profiles_dir()
        .join(&profile_path)
        .join("modpack.mrpack");
    
    // Check if file exists
    if !modpack_path.exists() {
        return Err(ApiError::not_found("Modpack not found for this instance"));
    }
    
    // Read the file
    let bytes = tokio::fs::read(&modpack_path).await
        .map_err(|e| ApiError::internal(&format!("Failed to read modpack: {}", e)))?;
    
    // Build response with appropriate headers
    let response = axum::response::Response::builder()
        .header("Content-Type", "application/zip")
        .header("Content-Disposition", format!("attachment; filename=\"modpack.mrpack\""))
        .body(axum::body::Body::from(bytes))
        .map_err(|e| ApiError::internal(&format!("Response error: {}", e)))?;
    
    Ok(response)
}

/// Get modpack metadata for an instance
pub async fn get_modpack_metadata(
    Path(instance_id): Path<String>,
    State(_registry): State<Arc<ServiceRegistry>>,
) -> Result<Json<ModpackMetadata>, ApiError> {
    let theseus_state = TheseusState::get().await
        .map_err(|e| ApiError::internal(&format!("Theseus state error: {}", e)))?;
    
    // Construct the modpack path
    let profile_path = format!("instance_{}", instance_id);
    let modpack_path = theseus_state.directories.profiles_dir()
        .join(&profile_path)
        .join("modpack.mrpack");
    
    // Check if file exists
    if !modpack_path.exists() {
        return Err(ApiError::not_found("Modpack not found for this instance"));
    }
    
    // Read and extract metadata
    let bytes = tokio::fs::read(&modpack_path).await
        .map_err(|e| ApiError::internal(&format!("Failed to read modpack: {}", e)))?;
    
    let metadata = extract_metadata(&bytes::Bytes::from(bytes)).await
        .map_err(|e| ApiError::internal(&format!("Failed to parse modpack: {}", e)))?;
    
    let loader = crate::theseus::pack::get_loader_from_dependencies(&metadata.dependencies);
    let game_version = crate::theseus::pack::get_game_version(&metadata.dependencies)
        .unwrap_or_else(|| "unknown".to_string());
    
    Ok(Json(ModpackMetadata {
        name: metadata.name,
        version_id: metadata.version_id,
        game_version,
        loader: loader.as_str().to_string(),
        summary: metadata.summary,
        file_count: metadata.files.len() as u32,
    }))
}

/// Response from modpack upload
#[derive(Serialize)]
pub struct ModpackUploadResponse {
    pub instance_id: String,
    pub name: String,
    pub game_version: String,
    pub loader: String,
    pub loader_version: Option<String>,
    pub summary: Option<String>,
}

/// Modpack metadata response
#[derive(Serialize)]
pub struct ModpackMetadata {
    pub name: String,
    pub version_id: String,
    pub game_version: String,
    pub loader: String,
    pub summary: Option<String>,
    pub file_count: u32,
}
