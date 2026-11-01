//! A server's icon, at `/v1/servers/{id}/icon`. Mirrors `ArchonIconsV1Module` in
//! `packages/api-client/src/modules/archon/icons/v1.ts`.
//!
//! - `GET`: the 64x64 game icon as PNG, or with `?original=true` the display icon, up to 512x512.
//!   Viewers and up. 404 when the server has no icon.
//! - `PUT`: the body is a PNG, JPEG, GIF, or WebP image. Editors and up.
//! - `DELETE`: removes the icon. Editors and up. 404 when there was none.
//!
//! Both files live in the server folder: `server-icon.png` is the one Minecraft shows in the
//! multiplayer list from its next start, and `server-icon-original.png` is what the App shows. A
//! `server-icon.png` that came with a pack or was placed by hand is the display icon too.

use std::{io::Cursor, sync::Arc};

use axum::{
    body::{Body, Bytes},
    extract::{Path, Query, State},
    http::{StatusCode, header},
    response::IntoResponse,
};
use image::{
    DynamicImage, ImageDecoder, ImageFormat, ImageReader, imageops::FilterType,
    metadata::Orientation,
};
use serde::Deserialize;

use super::{
    AppState,
    activity::Action,
    error::{ApiError, ErrorCode},
    files::{blocking, changing, folder, locked},
};
use crate::{
    access::{Caller, Level, activity},
    servers::{ServerError, records},
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

/// The owner passes `require` for any id, so a missing server is found here.
async fn exists(state: &AppState, id: &str) -> Result<(), ApiError> {
    records::get(&state.db, id)
        .await?
        .ok_or_else(|| ServerError::NotFound(id.into()))?;
    Ok(())
}

fn no_icon() -> ApiError {
    ApiError::new(ErrorCode::NotFound, "This server has no icon.")
}

/// `GET /v1/servers/{id}/icon`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(query): Query<IconQuery>,
) -> Result<impl IntoResponse, ApiError> {
    caller.require(&state.db, &id, Level::Viewer).await?;
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
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    body: Bytes,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    exists(&state, &id).await?;
    let (game, original) = blocking(move || icons(&body)).await?;
    let (folder, busy) = changing(&state, &id)?;
    locked(&busy, move || {
        folder.write(ORIGINAL_ICON, &mut original.as_slice())?;
        folder.write(GAME_ICON, &mut game.as_slice())?;
        Ok(())
    })
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(StatusCode::NO_CONTENT)
}

/// The game icon and the original as PNG files. The game icon is cropped to a square from the
/// middle; the original keeps its shape and only shrinks.
fn icons(image: &[u8]) -> Result<(Vec<u8>, Vec<u8>), ApiError> {
    let invalid = || {
        ApiError::new(
            ErrorCode::InvalidRequest,
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
            ErrorCode::InvalidRequest,
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
            ApiError::new(ErrorCode::Internal, error.to_string())
        })?;
    Ok(bytes)
}

/// `DELETE /v1/servers/{id}/icon`.
pub async fn delete(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
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
            folder.remove(name)?;
        }
        Ok(())
    })
    .await?;
    activity::log(&state.db, &caller, Some(&id), Action::SettingsChanged).await;
    Ok(StatusCode::NO_CONTENT)
}
