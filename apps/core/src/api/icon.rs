//! A server's icon, at `/v1/servers/{id}/icon`. Mirrors `ArchonIconsV1Module` in
//! `packages/api-client/src/modules/archon/icons/v1.ts`.
//!
//! - `GET`: the 64x64 game icon as PNG, or with `?original=true` the display icon, up to 512x512.
//!   Viewers and up. 404 when the server has no icon.
//! - `GET /metadata`: the game icon's hash and the icon editor settings it was made from.
//!   Viewers and up. 404 when the server has no icon.
//! - `PUT`: the body is a PNG, JPEG, GIF, or WebP image. Editors and up. Answers with the
//!   metadata.
//! - `PUT /generated`: an icon made in the icon editor, as the parts `image` and `config` (JSON
//!   `IconConfig`). Editors and up. Answers with the metadata.
//! - `DELETE`: removes the icon. Editors and up. 404 when there was none.
//!
//! Both files live in the server folder: `server-icon.png` is the one Minecraft shows in the
//! multiplayer list from its next start, and `server-icon-original.png` is what the App shows. A
//! `server-icon.png` that came with a pack or was placed by hand is the display icon too.
//!
//! The icon editor settings are kept in the server's row with the hash of the game icon they
//! made, so an icon replaced through the files routes drops them.

use std::{io::Cursor, sync::Arc};

use axum::{
    Json,
    body::{Body, Bytes},
    extract::{Multipart, Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use contracts::{ApiError, ErrorCode, core::Action};
use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, imageops::FilterType,
    metadata::Orientation,
};
use modrinth_content_management::icons::IconConfig;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{
    CoreState,
    files::{blocking, changing, folder, locked},
    internal,
};
use crate::{
    access::{Caller, Permission, activity},
    servers::ServerError,
    state::servers::adapters::sqlite::server_rows,
};

const GAME_ICON: &str = "server-icon.png";
const ORIGINAL_ICON: &str = "server-icon-original.png";
/// Minecraft only takes a 64x64 PNG.
const GAME_SIZE: u32 = 64;
const ORIGINAL_MAX: u32 = 512;
/// Larger uploads are refused before they are decoded.
const MAX_SOURCE_SIDE: u32 = 4096;

/// Query of `GET`. Mirrors the `original` param of `ArchonIconsV1Module.get`.
#[derive(Debug, Clone, Deserialize)]
pub struct IconQuery {
    #[serde(default)]
    pub original: bool,
}

/// A server's icon, as `Archon.Icons.v1.ServerIcon`.
#[derive(Debug, Clone, Serialize)]
pub struct ServerIcon {
    /// SHA-256 hex digest of the 64x64 game icon.
    pub hash: String,
    pub config: Option<IconConfig>,
}

/// The owner passes `require` for any id, so a missing server is found here.
async fn exists(state: &CoreState, id: &str) -> Result<(), ApiError> {
    server_rows::get(&state.db, id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    Ok(())
}

fn no_icon() -> ApiError {
    ApiError::new(ErrorCode::NotFound, "This server has no icon.")
}

/// `GET /v1/servers/{id}/icon`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path(id): Path<String>,
    Query(query): Query<IconQuery>,
) -> Result<impl IntoResponse, ApiError> {
    state.access.require(&caller, &id, Permission::View).await?;
    exists(&state, &id).await?;
    let folder = folder(&state, &id)?;
    let names: &[&str] = if query.original {
        &[ORIGINAL_ICON, GAME_ICON]
    } else {
        &[GAME_ICON]
    };
    // Streamed: the file routes can put a file of any size at these names.
    let file = blocking(move || {
        names
            .iter()
            .find_map(|name| folder.open_file(name).ok().flatten())
            .map(cap_std::fs::File::into_std)
            .ok_or_else(no_icon)
    })
    .await?;
    let stream =
        tokio_util::io::ReaderStream::new(tokio::fs::File::from_std(file));
    Ok((
        [(header::CONTENT_TYPE, "image/png")],
        Body::from_stream(stream),
    ))
}

/// `PUT /v1/servers/{id}/icon`.
pub async fn set(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<Json<ServerIcon>, ApiError> {
    state.access.require(&caller, &id, Permission::Edit).await?;
    exists(&state, &id).await?;
    write(&state, &caller, &id, body, None).await.map(Json)
}

/// `PUT /v1/servers/{id}/icon/generated`.
pub async fn set_generated(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path(id): Path<String>,
    mut form: Multipart,
) -> Result<Json<ServerIcon>, ApiError> {
    state.access.require(&caller, &id, Permission::Edit).await?;
    exists(&state, &id).await?;
    let bad_form = |error: axum::extract::multipart::MultipartError| {
        ApiError::new(ErrorCode::InvalidInput, error.body_text())
    };
    let (mut image, mut config) = (None, None);
    while let Some(field) = form.next_field().await.map_err(bad_form)? {
        match field.name() {
            Some("image") => {
                image = Some(field.bytes().await.map_err(bad_form)?)
            }
            Some("config") => {
                config = Some(field.text().await.map_err(bad_form)?)
            }
            _ => {}
        }
    }
    let (Some(image), Some(config)) = (image, config) else {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            "The upload needs an image and a config.",
        ));
    };
    let config: IconConfig = serde_json::from_str(&config)
        .ok()
        .filter(|config: &IconConfig| config.validate().is_ok())
        .ok_or_else(|| {
            ApiError::new(ErrorCode::InvalidInput, "Invalid icon config.")
        })?;
    write(&state, &caller, &id, image, Some(config))
        .await
        .map(Json)
}

/// Writes both icon files from `image` and keeps `config` with the game icon's hash.
async fn write(
    state: &CoreState,
    caller: &Caller,
    id: &str,
    image: Bytes,
    config: Option<IconConfig>,
) -> Result<ServerIcon, ApiError> {
    let (game, original) = blocking(move || icons(&image)).await?;
    let hash = hex::encode(Sha256::digest(&game));
    let (folder, busy) = changing(state, id)?;
    locked(&busy, move || {
        folder
            .write(ORIGINAL_ICON, &mut original.as_slice())
            .map_err(internal)?;
        folder
            .write(GAME_ICON, &mut game.as_slice())
            .map_err(internal)?;
        Ok(())
    })
    .await?;
    let stored = config.as_ref().map(|config| {
        serde_json::to_string(config).expect("an icon config is plain data")
    });
    server_rows::set_icon_config(
        &state.db,
        id,
        stored.as_deref().map(|config| (config, hash.as_str())),
    )
    .await
    .map_err(internal)?;
    activity::log(&state.db, caller, Some(id), Action::SettingsChanged).await;
    Ok(ServerIcon { hash, config })
}

/// `GET /v1/servers/{id}/icon/metadata`.
pub async fn metadata(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path(id): Path<String>,
) -> Result<Json<ServerIcon>, ApiError> {
    state.access.require(&caller, &id, Permission::View).await?;
    exists(&state, &id).await?;
    let folder = folder(&state, &id)?;
    let game = blocking(move || {
        folder
            .open_file(GAME_ICON)
            .map_err(internal)?
            .ok_or_else(no_icon)
            .and_then(|file| {
                let mut bytes = Vec::new();
                std::io::Read::read_to_end(&mut file.into_std(), &mut bytes)
                    .map_err(internal)?;
                Ok(bytes)
            })
    })
    .await?;
    let hash = hex::encode(Sha256::digest(&game));
    let config = server_rows::icon_config(&state.db, &id)
        .await
        .map_err(internal)?
        .filter(|(_, made)| *made == hash)
        .and_then(|(config, _)| serde_json::from_str(&config).ok());
    Ok(Json(ServerIcon { hash, config }))
}

/// The game icon and the original as PNG files. The game icon is cropped to a square from the
/// middle; the original keeps its shape and only shrinks.
fn icons(image: &[u8]) -> Result<(Vec<u8>, Vec<u8>), ApiError> {
    let invalid = || {
        ApiError::new(
            ErrorCode::InvalidInput,
            "Upload a PNG, JPEG, GIF, or WebP image.",
        )
    };
    let reader = || {
        let reader = ImageReader::new(Cursor::new(image))
            .with_guessed_format()
            .map_err(|_| invalid())?;
        match reader.format() {
            Some(
                ImageFormat::Png
                | ImageFormat::Jpeg
                | ImageFormat::Gif
                | ImageFormat::WebP,
            ) => Ok(reader),
            _ => Err(invalid()),
        }
    };
    let (width, height) = reader()?.into_dimensions().map_err(|_| invalid())?;
    if width > MAX_SOURCE_SIDE || height > MAX_SOURCE_SIDE {
        return Err(ApiError::new(
            ErrorCode::InvalidInput,
            format!(
                "Icons may be at most {MAX_SOURCE_SIDE}x{MAX_SOURCE_SIDE} pixels."
            ),
        ));
    }
    // Phone photos are often stored sideways, with the turn in their metadata.
    let mut decoder = reader()?.into_decoder().map_err(|_| invalid())?;
    let orientation =
        decoder.orientation().unwrap_or(Orientation::NoTransforms);
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|_| invalid())?;
    image.apply_orientation(orientation);
    let original = if width > ORIGINAL_MAX || height > ORIGINAL_MAX {
        image.resize(ORIGINAL_MAX, ORIGINAL_MAX, FilterType::Lanczos3)
    } else {
        image
    };
    let game =
        original.resize_to_fill(GAME_SIZE, GAME_SIZE, FilterType::Lanczos3);
    Ok((png(&game)?, png(&original)?))
}

fn png(image: &DynamicImage) -> Result<Vec<u8>, ApiError> {
    let mut bytes = Vec::new();
    image
        .write_to(&mut Cursor::new(&mut bytes), ImageFormat::Png)
        .map_err(|error| {
            ApiError::new(ErrorCode::InternalError, error.to_string())
        })?;
    Ok(bytes)
}

/// `DELETE /v1/servers/{id}/icon`.
pub async fn delete(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.access.require(&caller, &id, Permission::Edit).await?;
    exists(&state, &id).await?;
    let (folder, busy) = changing(&state, &id)?;
    locked(&busy, move || {
        // Only plain files: `remove` would also take a folder by that name, and its contents.
        let icons: Vec<_> = [GAME_ICON, ORIGINAL_ICON]
            .into_iter()
            .filter(|name| {
                folder.metadata(name).is_ok_and(|meta| meta.is_file())
            })
            .collect();
        if icons.is_empty() {
            return Err(no_icon());
        }
        for name in icons {
            folder.remove(name).map_err(internal)?;
        }
        Ok(())
    })
    .await?;
    server_rows::set_icon_config(&state.db, &id, None)
        .await
        .map_err(internal)?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(StatusCode::NO_CONTENT)
}
