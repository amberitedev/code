use std::sync::Arc;

use crate::{
    application::{social_models::SocialError, state::AppState},
    domain::{event::Event, modpack::PackFormat},
    infrastructure::minecraft::mrpack::extract_metadata,
};
use bytes::Bytes;

pub async fn metadata_from_bytes(
    archive: &Bytes,
) -> Result<PackFormat, SocialError> {
    let dir = tempfile::tempdir()?;
    let path = dir.path().join("snapshot.mrpack");
    tokio::fs::write(&path, archive.as_ref()).await?;
    Ok(extract_metadata(&path).await?)
}

pub async fn mark_event_failed(
    state: &Arc<AppState>,
    profile_id: &str,
    event_id: &str,
    message: &str,
) -> Result<(), SocialError> {
    sqlx::query(
        "UPDATE sync_events SET status = 'failed', message = ? WHERE id = ?",
    )
    .bind(message)
    .bind(event_id)
    .execute(&state.pool)
    .await?;
    state.broadcaster.send(Event::SyncEventStatusChanged {
        profile_id: profile_id.to_string(),
        event_id: event_id.to_string(),
        status: "failed".into(),
        message: Some(message.to_string()),
    });
    Ok(())
}

pub fn detect_loader(metadata: &PackFormat) -> Option<String> {
    let deps = &metadata.dependencies;
    if deps.contains_key("fabric-loader") {
        Some("fabric".into())
    } else if deps.contains_key("quilt-loader") {
        Some("quilt".into())
    } else if deps.contains_key("forge") {
        Some("forge".into())
    } else if deps.contains_key("neoforge") {
        Some("neoforge".into())
    } else {
        Some("vanilla".into())
    }
}
