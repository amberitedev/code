//! Compatibility with public hash endpoints when Modrinth's firewall blocks bulk requests.
use crate::state::Version;
use crate::util::fetch::{FetchSemaphore, fetch_json};
use futures::{StreamExt, TryStreamExt};
use reqwest::{Method, StatusCode};
use sqlx::SqlitePool;
use std::collections::HashMap;

pub(crate) async fn versions_by_hash(
    hashes: &[String],
    semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<HashMap<String, Version>> {
    let result = fetch_json(
        Method::POST,
        concat!(env!("MODRINTH_API_URL"), "version_files"),
        None,
        Some(serde_json::json!({ "algorithm": "sha1", "hashes": hashes })),
        Some("/v2/version_files"),
        semaphore,
        pool,
    )
    .await;
    match result {
        Err(error) if is_firewall_rejection(&error) => {}
        result => return result,
    }

    futures::stream::iter(hashes.to_vec())
        .map(|hash| async move {
            let version = optional_version(
                Method::GET,
                &format!(
                    "{}version_file/{hash}?algorithm=sha1",
                    env!("MODRINTH_API_URL")
                ),
                None,
                "/v2/version_file/:hash",
                semaphore,
                pool,
            )
            .await?;
            Ok(version.map(|version| (hash, version)))
        })
        .buffer_unordered(4)
        .try_filter_map(|entry| async { Ok(entry) })
        .try_collect()
        .await
}

fn is_firewall_rejection(error: &crate::Error) -> bool {
    // JSON API permission errors use LabrinthError and must not trigger this fallback.
    matches!(error.raw.as_ref(), crate::ErrorKind::FetchError(error)
        if error.status() == Some(StatusCode::FORBIDDEN))
}

async fn optional_version(
    method: Method,
    url: &str,
    body: Option<serde_json::Value>,
    route: &'static str,
    semaphore: &FetchSemaphore,
    pool: &SqlitePool,
) -> crate::Result<Option<Version>> {
    match fetch_json(method, url, None, body, Some(route), semaphore, pool)
        .await
    {
        Ok(version) => Ok(Some(version)),
        Err(error)
            if matches!(error.raw.as_ref(), crate::ErrorKind::FetchError(error)
            if error.status() == Some(StatusCode::NOT_FOUND)) =>
        {
            Ok(None)
        }
        Err(error)
            if matches!(error.raw.as_ref(), crate::ErrorKind::LabrinthError(error)
            if error.status == Some(404)) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
