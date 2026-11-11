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
    fetch_status(error) == Some(StatusCode::FORBIDDEN)
}

/// The HTTP status of a plain (non-JSON) fetch failure.
fn fetch_status(error: &crate::Error) -> Option<StatusCode> {
    error
        .chain()
        .find_map(|cause| cause.downcast_ref::<reqwest::Error>())
        .and_then(reqwest::Error::status)
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
            if fetch_status(&error) == Some(StatusCode::NOT_FOUND)
                || error
                    .downcast_ref::<crate::LabrinthError>()
                    .is_some_and(|error| error.status == Some(404)) =>
        {
            Ok(None)
        }
        Err(error) => Err(error),
    }
}
