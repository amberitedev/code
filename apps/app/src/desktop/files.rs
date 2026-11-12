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
    server::endpoints::files::{
        CacheRemoteIcon, FileStat, GetLocalFile, GetThumbnail, ReadLocalFile,
        StatLocalFile,
    },
};

pub fn routes(engine: EngineHandle) -> Router {
    let (file_path, file) = route::<GetLocalFile, _, _, _>(local_file);
    let (thumbnail_path, thumbnail) = route::<GetThumbnail, _, _, _>(thumbnail);
    let (remote_icon_path, remote_icon) =
        route::<CacheRemoteIcon, _, _, _>(cache_remote_icon);
    let (read_path, read) = route::<ReadLocalFile, _, _, _>(read_file);
    let (stat_path, stat) = route::<StatLocalFile, _, _, _>(stat_file);
    Router::new()
        .route(file_path, file)
        .route(thumbnail_path, thumbnail)
        .route(remote_icon_path, remote_icon)
        .route(read_path, read)
        .route(stat_path, stat)
        .with_state(engine)
}

async fn read_file(
    Req { query, .. }: Req<ReadLocalFile>,
) -> Result<Response, ApiError> {
    let metadata = tokio::fs::metadata(&query.path)
        .await
        .map_err(|error| file_error(&query.path, &error))?;
    if !metadata.is_file() {
        return Err(not_a_file(&query.path));
    }
    let file = tokio::fs::File::open(&query.path)
        .await
        .map_err(|error| file_error(&query.path, &error))?;
    Ok((
        [(CONTENT_LENGTH, HeaderValue::from(metadata.len()))],
        Body::from_stream(ReaderStream::new(file)),
    )
        .into_response())
}

async fn stat_file(
    Req { query, .. }: Req<StatLocalFile>,
) -> Result<Json<FileStat>, ApiError> {
    let metadata = tokio::fs::metadata(&query.path)
        .await
        .map_err(|error| file_error(&query.path, &error))?;
    if !metadata.is_file() {
        return Err(not_a_file(&query.path));
    }
    let name = query.path.file_name().map_or_else(
        || "file".to_owned(),
        |name| name.to_string_lossy().into_owned(),
    );
    Ok(Json(FileStat {
        name,
        size: metadata.len(),
    }))
}

fn file_error(path: &Path, error: &std::io::Error) -> ApiError {
    let code = if error.kind() == std::io::ErrorKind::NotFound {
        ErrorCode::NotFound
    } else {
        ErrorCode::InternalError
    };
    ApiError::new(code, format!("Could not read {}: {error}", path.display()))
}

fn not_a_file(path: &Path) -> ApiError {
    ApiError::new(
        ErrorCode::InvalidInput,
        format!("{} is not a file.", path.display()),
    )
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
    use std::path::Path;

    use axum::http::{
        HeaderMap, StatusCode,
        header::{CACHE_CONTROL, ETAG, IF_NONE_MATCH},
    };
    use contracts::{Empty, Endpoint, ErrorCode, Json, server::Req};
    use http_body_util::BodyExt;

    use super::{is_cached_icon, read_file, serve, stat_file};
    use crate::server::endpoints::files::LocalFileQuery;

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

    fn query<E>(path: &Path) -> Req<E>
    where
        E: Endpoint<Params = (), Query = LocalFileQuery, Body = Empty>,
    {
        Req {
            params: (),
            query: LocalFileQuery {
                path: path.to_owned(),
            },
            body: (),
        }
    }

    #[tokio::test]
    async fn fs_routes_read_a_file_and_refuse_a_folder() {
        let folder = tempfile::tempdir().unwrap();
        let path = folder.path().join("skin.png");
        std::fs::write(&path, b"bytes").unwrap();

        let response = read_file(query(&path)).await.unwrap();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(&body[..], b"bytes");
        let Json(stat) = stat_file(query(&path)).await.unwrap();
        assert_eq!((stat.name.as_str(), stat.size), ("skin.png", 5));

        let rows = [
            (folder.path().to_owned(), ErrorCode::InvalidInput),
            (folder.path().join("missing"), ErrorCode::NotFound),
        ];
        for (path, code) in rows {
            let read = read_file(query(&path)).await.err().unwrap();
            let stat = stat_file(query(&path)).await.err().unwrap();
            assert_eq!((read.error, stat.error), (code, code), "{path:?}");
        }
    }
}
