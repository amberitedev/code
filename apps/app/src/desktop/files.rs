//! Serves `crate::server::endpoints::files`.

use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use axum::{
    Router,
    body::Body,
    extract::State,
    http::{
        HeaderMap, HeaderValue, StatusCode,
        header::{
            CACHE_CONTROL, CONTENT_LENGTH, CONTENT_TYPE, ETAG, IF_NONE_MATCH,
        },
    },
    response::{IntoResponse, Response},
};
use contracts::{
    ApiError, ErrorCode, Json,
    server::{Req, route},
};
use tokio_util::io::ReaderStream;

use super::thumbnails;
use crate::{
    engine_handle::EngineHandle,
    server::endpoints::files::{CacheRemoteIcon, GetLocalFile, GetThumbnail},
};

pub fn routes(engine: EngineHandle) -> Router {
    let (file_path, file) = route::<GetLocalFile, _, _, _>(local_file);
    let (thumbnail_path, thumbnail) = route::<GetThumbnail, _, _, _>(thumbnail);
    let (remote_icon_path, remote_icon) =
        route::<CacheRemoteIcon, _, _, _>(cache_remote_icon);
    Router::new()
        .route(file_path, file)
        .route(thumbnail_path, thumbnail)
        .route(remote_icon_path, remote_icon)
        .with_state(engine)
}

async fn local_file(
    State(engine): State<EngineHandle>,
    headers: HeaderMap,
    Req { query, .. }: Req<GetLocalFile>,
) -> Response {
    let Ok(path) = tokio::fs::canonicalize(&query.path).await else {
        return not_found();
    };
    let icons = engine.get().await.directories.caches_dir().join("icons");
    let immutable = is_cached_icon(&path, &icons).await;
    serve(&path, immutable, &headers).await
}

async fn thumbnail(
    State(engine): State<EngineHandle>,
    headers: HeaderMap,
    Req { query, .. }: Req<GetThumbnail>,
) -> Response {
    let Ok(path) = tokio::fs::canonicalize(&query.path).await else {
        return not_found();
    };
    let cache = engine
        .get()
        .await
        .directories
        .caches_dir()
        .join("thumbnails-v1");
    let image =
        match thumbnails::thumbnail(path.clone(), cache, query.size).await {
            Ok(thumbnail) => thumbnail,
            Err(error) => {
                tracing::warn!(%error, "Could not create image thumbnail");
                path
            }
        };
    serve(&image, false, &headers).await
}

async fn cache_remote_icon(
    State(engine): State<EngineHandle>,
    Req { body, .. }: Req<CacheRemoteIcon>,
) -> Result<Json<PathBuf>, ApiError> {
    let state = engine.get().await;
    engine::instance::cache_remote_icon(&state, &body.source)
        .await
        .map(Json)
        .map_err(|error| {
            ApiError::new(ErrorCode::InternalError, error.to_string())
        })
}

/// The icon cache names each file by the SHA-1 of its bytes, so its files never change.
async fn is_cached_icon(path: &Path, icons: &Path) -> bool {
    let hashed = path
        .file_name()
        .and_then(OsStr::to_str)
        .and_then(|name| name.strip_suffix(".png"))
        .is_some_and(|hash| {
            hash.len() == 40
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
        });
    hashed
        && match (path.parent(), tokio::fs::canonicalize(icons).await) {
            (Some(parent), Ok(icons)) => parent == icons,
            _ => false,
        }
}

/// An `immutable` file is cached for good; any other is revalidated against an ETag of its length
/// and modification time, so an image edited in place loads again.
async fn serve(path: &Path, immutable: bool, request: &HeaderMap) -> Response {
    let Ok(file) = tokio::fs::File::open(path).await else {
        return not_found();
    };
    let Ok(metadata) = file.metadata().await else {
        return not_found();
    };
    if !metadata.is_file() {
        return not_found();
    }
    let modified = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
        .unwrap_or_default();
    let etag = format!("\"{:x}-{:x}\"", metadata.len(), modified.as_nanos());
    let mut response = if is_fresh(request, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        (
            [
                (CONTENT_TYPE, HeaderValue::from_static(content_type(path))),
                (CONTENT_LENGTH, HeaderValue::from(metadata.len())),
            ],
            Body::from_stream(ReaderStream::new(file)),
        )
            .into_response()
    };
    let headers = response.headers_mut();
    headers.insert(
        CACHE_CONTROL,
        HeaderValue::from_static(if immutable {
            "max-age=31536000, immutable"
        } else {
            "no-cache"
        }),
    );
    if let Ok(etag) = HeaderValue::from_str(&etag) {
        headers.insert(ETAG, etag);
    }
    response
}

fn is_fresh(request: &HeaderMap, etag: &str) -> bool {
    request
        .get(IF_NONE_MATCH)
        .and_then(|tags| tags.to_str().ok())
        .is_some_and(|tags| {
            tags.split(',').map(str::trim).any(|tag| {
                tag == "*" || tag.strip_prefix("W/").unwrap_or(tag) == etag
            })
        })
}

/// Other types are left to the browser to sniff.
fn content_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(OsStr::to_str)
        .map(str::to_ascii_lowercase);
    match extension.as_deref() {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("svg") => "image/svg+xml",
        _ => "application/octet-stream",
    }
}

/// Says nothing about the file: whether it exists, is a folder or cannot be read.
fn not_found() -> Response {
    ApiError::new(ErrorCode::NotFound, "Not found.").into_response()
}

#[cfg(test)]
mod tests {
    use axum::http::{
        HeaderMap, StatusCode,
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
    };

    use super::{is_cached_icon, serve};

    #[tokio::test]
    async fn a_matching_etag_is_not_modified_until_the_file_changes() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("icon.png");
        std::fs::write(&path, b"one").unwrap();

        let response = serve(&path, false, &HeaderMap::new()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[CACHE_CONTROL], "no-cache");
        let etag = response.headers()[ETAG].clone();

        let mut revalidate = HeaderMap::new();
        revalidate.insert(IF_NONE_MATCH, etag);
        let response = serve(&path, false, &revalidate).await;
        assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

        std::fs::write(&path, b"edited").unwrap();
        let response = serve(&path, false, &revalidate).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn only_hash_named_icons_are_immutable() {
        let folder = tempfile::tempdir().unwrap();
        let icons = folder.path().join("icons");
        std::fs::create_dir(&icons).unwrap();
        let hash = "0123456789abcdef0123456789abcdef01234567";
        let rows = [
            (icons.join(format!("{hash}.png")), true),
            (icons.join(format!("remote-{hash}.txt")), false),
            (folder.path().join(format!("{hash}.png")), false),
        ];
        for (path, immutable) in rows {
            std::fs::write(&path, b"icon").unwrap();
            let path = std::fs::canonicalize(path).unwrap();
            assert_eq!(
                is_cached_icon(&path, &icons).await,
                immutable,
                "{}",
                path.display()
            );
        }

        let path =
            std::fs::canonicalize(icons.join(format!("{hash}.png"))).unwrap();
        let response = serve(&path, true, &HeaderMap::new()).await;
        assert_eq!(
            response.headers()[CACHE_CONTROL],
            "max-age=31536000, immutable"
        );
    }

    #[tokio::test]
    async fn a_folder_or_a_missing_file_is_not_found() {
        let folder = tempfile::tempdir().unwrap();
        for path in [folder.path().to_owned(), folder.path().join("missing")] {
            let response = serve(&path, false, &HeaderMap::new()).await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
}
