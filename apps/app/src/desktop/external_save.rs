//! Serves `SaveExternalFile` and `SaveInstanceFileAs` of `crate::server::endpoints::ops`: saves
//! to a path the person picked outside the App.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{Router, extract::State, http::StatusCode};
use contracts::{
    ApiError, ErrorCode,
    server::{Req, route},
};
use engine::{Engine, routes::api_error};
use tauri_plugin_http::reqwest;
use tokio::io::AsyncWriteExt;

use crate::server::{
    AppState,
    endpoints::ops::{
        ExternalSave, OpResponse, SaveExternalFile, SaveInstanceFileAs,
    },
    ops::{Op, Ops},
};

pub fn routes() -> Router<AppState> {
    let (external_path, external) =
        route::<SaveExternalFile, _, _, _>(save_external_file);
    let (instance_path, instance) =
        route::<SaveInstanceFileAs, _, _, _>(save_instance_file_as);
    Router::new()
        .route(external_path, external)
        .route(instance_path, instance)
}

async fn save_external_file(
    State(ops): State<Arc<Ops>>,
    Req { body, .. }: Req<SaveExternalFile>,
) -> Result<StatusCode, ApiError> {
    let url = url::Url::parse(&body.url)
        .ok()
        .filter(|url| matches!(url.scheme(), "http" | "https"))
        .ok_or_else(|| {
            ApiError::new(ErrorCode::InvalidInput, "Unsupported download URL")
        })?;
    ops.start(body.op.clone(), |op| async move {
        op.until_cancelled(download(&op, url, body)).await
    })
}

async fn download(
    op: &Op,
    url: url::Url,
    save: ExternalSave,
) -> Result<Option<OpResponse>, ApiError> {
    op.progress(Some("waiting"), 0, None);
    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_secs(30))
        .read_timeout(Duration::from_secs(60))
        .build()
        .map_err(save_failed)?;
    let mut request = client.get(url);
    for (name, value) in save.headers {
        request = request.header(name, value);
    }
    let mut response = request.send().await.map_err(save_failed)?;
    let status = response.status();
    if !status.is_success() {
        return Ok(Some(OpResponse {
            status: status.as_u16(),
            body: response.text().await.unwrap_or_default(),
        }));
    }
    let total = response.content_length();
    let mut done = 0;
    let mut last_progress = Instant::now();
    op.progress(Some("downloading"), done, total);
    let parent = save.path.parent().ok_or_else(|| {
        ApiError::new(ErrorCode::InvalidInput, "Invalid save destination")
    })?;
    let (file, temporary) = tempfile::NamedTempFile::new_in(parent)
        .map_err(save_failed)?
        .into_parts();
    let mut file = tokio::fs::File::from_std(file);
    while let Some(chunk) = response.chunk().await.map_err(save_failed)? {
        file.write_all(&chunk).await.map_err(save_failed)?;
        done += chunk.len() as u64;
        if last_progress.elapsed() >= Duration::from_millis(200) {
            op.progress(Some("downloading"), done, total);
            last_progress = Instant::now();
        }
    }
    op.progress(Some("saving"), done, total);
    file.flush().await.map_err(save_failed)?;
    file.sync_all().await.map_err(save_failed)?;
    drop(file);
    temporary
        .persist(&save.path)
        .map_err(|error| save_failed(error.error))?;
    Ok(None)
}

fn save_failed(error: impl std::fmt::Display) -> ApiError {
    ApiError::new(ErrorCode::ExternalSaveFailed, error.to_string())
}

async fn save_instance_file_as(
    State(engine): State<Arc<Engine>>,
    State(ops): State<Arc<Ops>>,
    Req { body, .. }: Req<SaveInstanceFileAs>,
) -> Result<StatusCode, ApiError> {
    ops.start(body.op, |_| async move {
        engine::instance::save_instance_file_as(
            &engine,
            &body.instance_id,
            &body.path,
            &body.destination,
        )
        .await
        .map_err(api_error)?;
        Ok(None)
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use axum::routing::get;
    use tokio::sync::broadcast;

    use super::*;
    use crate::server::events::AppEvent;

    async fn save(url: String, path: std::path::PathBuf) -> AppEvent {
        let events = broadcast::Sender::new(16);
        let mut receiver = events.subscribe();
        let ops = Arc::new(Ops::new(events));
        let save = ExternalSave {
            op: "save".into(),
            path,
            url,
            headers: HashMap::new(),
        };
        let url = url::Url::parse(&save.url).unwrap();
        ops.start(save.op.clone(), |op| async move {
            download(&op, url, save).await
        })
        .unwrap();
        loop {
            let event = receiver.recv().await.unwrap();
            if matches!(event, AppEvent::OpEnded { .. }) {
                return event;
            }
        }
    }

    #[tokio::test]
    async fn an_unreachable_server_fails_with_external_save_failed() {
        let dir = tempfile::tempdir().unwrap();
        let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://{}/backup.zip", closed.local_addr().unwrap());
        drop(closed);

        let ended = save(url, dir.path().join("backup.zip")).await;

        assert!(matches!(
            ended,
            AppEvent::OpEnded { error: Some(error), response: None, .. }
                if error.error == ErrorCode::ExternalSaveFailed
        ));
    }

    #[tokio::test]
    async fn a_refused_download_saves_nothing_and_ends_with_its_status() {
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = Router::new().route(
            "/backup.zip",
            get(|| async { (StatusCode::UNAUTHORIZED, "expired") }),
        );
        tokio::spawn(async move { axum::serve(listener, server).await });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("backup.zip");

        let ended =
            save(format!("http://{address}/backup.zip"), path.clone()).await;

        assert!(matches!(
            ended,
            AppEvent::OpEnded { error: None, response: Some(OpResponse { status: 401, body }), .. }
                if body == "expired"
        ));
        assert!(!path.exists());
    }
}
