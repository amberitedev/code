use std::{path::PathBuf, sync::Arc};

use bytes::Bytes;

use crate::application::{social_models::SocialError, state::AppState};

pub async fn save_archive(
    state: &Arc<AppState>,
    snapshot_id: &str,
    data: Bytes,
) -> Result<PathBuf, SocialError> {
    let dir = state.config.data_dir.join("sync_archives");
    tokio::fs::create_dir_all(&dir).await?;
    let path = dir.join(format!("{snapshot_id}.mrpack"));
    tokio::fs::write(&path, data).await?;
    Ok(path)
}

pub async fn archive_for_snapshot(
    state: &Arc<AppState>,
    snapshot_id: &str,
) -> Result<PathBuf, SocialError> {
    let row: Option<(Option<String>, bool)> = sqlx::query_as(
        "SELECT archive_path, archived FROM sync_snapshots WHERE id = ?",
    )
    .bind(snapshot_id)
    .fetch_optional(&state.pool)
    .await?;
    let Some((Some(path), archived)) = row else {
        return Err(SocialError::NotFound);
    };
    if archived {
        return Err(SocialError::Invalid("snapshot archive was pruned".into()));
    }
    Ok(PathBuf::from(path))
}

pub async fn prune_old_archives(
    state: &Arc<AppState>,
    profile_id: &str,
) -> Result<(), SocialError> {
    let retain = state.config.sync_retain_count;
    if retain == 0 {
        return Ok(());
    }
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT id, archive_path FROM sync_snapshots \
		 WHERE profile_id = ? AND archived = 0 \
		 ORDER BY created_at DESC LIMIT -1 OFFSET ?",
    )
    .bind(profile_id)
    .bind(retain as i64)
    .fetch_all(&state.pool)
    .await?;
    for (id, archive_path) in rows {
        if let Some(path) = archive_path {
            let _ = tokio::fs::remove_file(path).await;
        }
        sqlx::query("UPDATE sync_snapshots SET archived = 1 WHERE id = ?")
            .bind(id)
            .execute(&state.pool)
            .await?;
    }
    Ok(())
}
