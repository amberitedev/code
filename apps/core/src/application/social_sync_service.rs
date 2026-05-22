use std::sync::Arc;

use bytes::Bytes;
use uuid::Uuid;

use crate::{
    application::{
        social_lookup_service,
        social_models::{
            CreateSyncProfileFromMrpackRequest, RegisterSyncProfileRequest,
            SocialError, SyncProfile, SyncSnapshotPublishResult,
            SyncVersionStatus,
        },
        state::AppState,
        sync_apply_service, sync_archive_service, sync_profile_support,
    },
    domain::{event::Event, modpack::PackFormat},
};

pub async fn register_sync_profile(
    state: &Arc<AppState>,
    req: RegisterSyncProfileRequest,
) -> Result<SyncProfile, SocialError> {
    let id = req.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let now = social_lookup_service::now();
    sqlx::query("INSERT INTO sync_profiles (id, client_profile_id, core_instance_id, name, game_version, loader, sync_enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(id) DO UPDATE SET client_profile_id = excluded.client_profile_id, core_instance_id = excluded.core_instance_id, name = excluded.name, game_version = excluded.game_version, loader = excluded.loader, sync_enabled = excluded.sync_enabled, updated_at = excluded.updated_at")
		.bind(&id).bind(req.client_profile_id).bind(req.core_instance_id).bind(req.name).bind(req.game_version).bind(req.loader)
		.bind(req.sync_enabled.unwrap_or(true)).bind(&now).bind(&now).execute(&state.pool).await?;
    social_lookup_service::get_sync_profile(state, &id).await
}

pub async fn publish_snapshot(
    state: &Arc<AppState>,
    profile_id: &str,
    owner_user_id: &str,
    archive: Bytes,
    notes: Option<String>,
) -> Result<SyncSnapshotPublishResult, SocialError> {
    let profile =
        social_lookup_service::get_sync_profile(state, profile_id).await?;
    let metadata = sync_profile_support::metadata_from_bytes(&archive).await?;
    store_snapshot(
        state,
        profile,
        owner_user_id.to_string(),
        archive,
        metadata,
        notes,
    )
    .await
}

pub async fn create_profile_from_mrpack(
    state: &Arc<AppState>,
    owner_user_id: &str,
    req: CreateSyncProfileFromMrpackRequest,
    archive: Bytes,
) -> Result<SyncSnapshotPublishResult, SocialError> {
    let metadata = sync_profile_support::metadata_from_bytes(&archive).await?;
    let id = Uuid::new_v4().to_string();
    let now = social_lookup_service::now();
    sqlx::query("INSERT INTO sync_profiles (id, client_profile_id, core_instance_id, name, game_version, loader, sync_enabled, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)")
		.bind(&id)
		.bind(req.client_profile_id)
		.bind(req.core_instance_id)
		.bind(req.name.unwrap_or_else(|| metadata.name.clone()))
		.bind(metadata.dependencies.get("minecraft").cloned())
		.bind(sync_profile_support::detect_loader(&metadata))
		.bind(req.sync_enabled.unwrap_or(true))
		.bind(&now)
		.bind(&now)
		.execute(&state.pool)
		.await?;
    let profile = social_lookup_service::get_sync_profile(state, &id).await?;
    store_snapshot(
        state,
        profile,
        owner_user_id.to_string(),
        archive,
        metadata,
        req.notes,
    )
    .await
}

async fn store_snapshot(
    state: &Arc<AppState>,
    profile: SyncProfile,
    author: String,
    archive: Bytes,
    metadata: PackFormat,
    notes: Option<String>,
) -> Result<SyncSnapshotPublishResult, SocialError> {
    let snapshot_id = Uuid::new_v4().to_string();
    let event_id = Uuid::new_v4().to_string();
    let now = social_lookup_service::now();
    let archive_path =
        sync_archive_service::save_archive(state, &snapshot_id, archive)
            .await?;
    let manifest = serde_json::to_string(&metadata)
        .map_err(|e| SocialError::Invalid(e.to_string()))?;

    sqlx::query("INSERT INTO sync_snapshots (id, profile_id, author_user_id, manifest_json, notes, created_at, archive_path, archived) VALUES (?, ?, ?, ?, ?, ?, ?, 0)")
		.bind(&snapshot_id)
		.bind(&profile.id)
		.bind(author)
		.bind(manifest)
		.bind(notes)
		.bind(&now)
		.bind(archive_path.to_string_lossy().to_string())
		.execute(&state.pool)
		.await?;
    sqlx::query("INSERT INTO sync_events (id, profile_id, snapshot_id, status, message, created_at) VALUES (?, ?, ?, 'applying', ?, ?)")
		.bind(&event_id)
		.bind(&profile.id)
		.bind(&snapshot_id)
		.bind("Applying sync snapshot.")
		.bind(&now)
		.execute(&state.pool)
		.await?;
    state.broadcaster.send(Event::SyncEventStatusChanged {
        profile_id: profile.id.clone(),
        event_id: event_id.clone(),
        status: "applying".into(),
        message: Some("Applying sync snapshot.".into()),
    });

    let diff_json = if let Some(instance_id) = &profile.core_instance_id {
        match sync_apply_service::apply_snapshot(
            state,
            instance_id,
            &archive_path,
            &metadata,
        )
        .await
        {
            Ok(diff) => Some(
                serde_json::to_string(&diff)
                    .map_err(|e| SocialError::Invalid(e.to_string()))?,
            ),
            Err(err) => {
                sync_profile_support::mark_event_failed(
                    state,
                    &profile.id,
                    &event_id,
                    &err.to_string(),
                )
                .await?;
                return Err(err);
            }
        }
    } else {
        None
    };
    let applied_at = social_lookup_service::now();
    sqlx::query("UPDATE sync_events SET status = 'completed', diff_json = ?, message = ?, applied_at = ? WHERE id = ?")
		.bind(diff_json)
		.bind("Snapshot stored and applied.")
		.bind(&applied_at)
		.bind(&event_id)
		.execute(&state.pool)
		.await?;
    sqlx::query("UPDATE sync_profiles SET last_snapshot_at = ?, current_snapshot_id = ?, updated_at = ? WHERE id = ?")
		.bind(&applied_at)
		.bind(&snapshot_id)
		.bind(&applied_at)
		.bind(&profile.id)
		.execute(&state.pool)
		.await?;
    sync_archive_service::prune_old_archives(state, &profile.id).await?;
    sync_profile_support::publish_update_message(
        state,
        &profile.id,
        profile.core_instance_id.as_deref(),
        &snapshot_id,
    )
    .await?;

    state.broadcaster.send(Event::SyncEventStatusChanged {
        profile_id: profile.id.clone(),
        event_id: event_id.clone(),
        status: "completed".into(),
        message: Some("Snapshot stored and applied.".into()),
    });
    state.broadcaster.send(Event::SyncProfileUpdated {
        profile_id: profile.id.clone(),
        snapshot_id: snapshot_id.clone(),
        instance_id: profile.core_instance_id.clone(),
    });

    Ok(SyncSnapshotPublishResult {
        profile: social_lookup_service::get_sync_profile(state, &profile.id)
            .await?,
        snapshot: sqlx::query_as("SELECT * FROM sync_snapshots WHERE id = ?")
            .bind(&snapshot_id)
            .fetch_one(&state.pool)
            .await?,
        event: sqlx::query_as("SELECT * FROM sync_events WHERE id = ?")
            .bind(&event_id)
            .fetch_one(&state.pool)
            .await?,
    })
}

pub async fn check_version(
    state: &Arc<AppState>,
    profile_id: &str,
) -> Result<SyncVersionStatus, SocialError> {
    let row: Option<(Option<String>, Option<String>)> = sqlx::query_as(
		"SELECT p.current_snapshot_id, s.created_at FROM sync_profiles p LEFT JOIN sync_snapshots s ON s.id = p.current_snapshot_id WHERE p.id = ?",
	)
	.bind(profile_id)
	.fetch_optional(&state.pool)
	.await?;
    let Some((current_snapshot_id, created_at)) = row else {
        return Err(SocialError::NotFound);
    };
    Ok(SyncVersionStatus {
        profile_id: profile_id.to_string(),
        current_snapshot_id,
        current_snapshot_created_at: created_at,
    })
}

pub async fn ensure_snapshot_in_profile(
    state: &Arc<AppState>,
    profile_id: &str,
    snapshot_id: &str,
) -> Result<(), SocialError> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sync_snapshots WHERE id = ? AND profile_id = ?",
    )
    .bind(snapshot_id)
    .bind(profile_id)
    .fetch_one(&state.pool)
    .await?;
    if count == 0 {
        return Err(SocialError::NotFound);
    }
    Ok(())
}
