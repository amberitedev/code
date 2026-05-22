use std::sync::Arc;

use bytes::Bytes;
use uuid::Uuid;

use crate::{
    application::{social_models::SocialError, state::AppState},
    domain::{event::Event, modpack::PackFormat},
    infrastructure::minecraft::mrpack::extract_metadata,
};

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

pub async fn publish_update_message(
    state: &Arc<AppState>,
    profile_id: &str,
    instance_id: Option<&str>,
    snapshot_id: &str,
) -> Result<(), SocialError> {
    let now = chrono::Utc::now();
    let expires_at = now + chrono::Duration::days(7);
    let payload = serde_json::json!({
        "profile_id": profile_id,
        "snapshot_id": snapshot_id,
        "instance_id": instance_id,
    });
    sqlx::query("INSERT INTO core_relay_messages (id, type, version, sender_id, recipient_id, payload, ack, status, created_at, expires_at) VALUES (?, 'sync_profile_updated', 1, ?, '*', ?, 'received', 'pending', ?, ?)")
		.bind(Uuid::new_v4().to_string())
		.bind(&state.core_id)
		.bind(payload.to_string())
		.bind(now.to_rfc3339())
		.bind(expires_at.to_rfc3339())
		.execute(&state.pool)
		.await?;
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
