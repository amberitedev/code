//! A server engine's record of the files it tracks: `instance_files` joined with the entry that
//! says where each came from. Rows of the pack's client-only files are `missing`.

use std::collections::HashSet;

use chrono::Utc;
use sqlx::{SqliteConnection, SqlitePool};
use uuid::Uuid;

use crate::api::pack::server::{
    EntrySides, ServerOn, SkippedFile, TrackedFile, project_type_of,
};
use crate::state::{ContentRequirement, ContentSourceKind, PendingChange};

#[derive(sqlx::FromRow)]
struct TrackedRow {
    relative_path: String,
    sha1: String,
    size: i64,
    pending: Option<String>,
    source_kind: String,
    project_id: Option<String>,
    version_id: Option<String>,
    enabled: bool,
    server_requirement: String,
    client_requirement: String,
    sides_unlocked: bool,
    server_override: bool,
}

const TRACKED: &str = "SELECT f.relative_path, f.sha1, f.size, f.pending, e.source_kind, \
     e.project_id, e.version_id, e.enabled, e.server_requirement, e.client_requirement, \
     e.sides_unlocked, e.server_override FROM instance_files f \
     JOIN instance_content_entries e ON e.file_id = f.id \
     WHERE f.instance_id = ? AND f.missing = 0";

impl TryFrom<TrackedRow> for (TrackedFile, EntrySides) {
    type Error = crate::Error;

    fn try_from(row: TrackedRow) -> crate::Result<Self> {
        Ok((
            TrackedFile {
                relative_path: row.relative_path,
                source_kind: ContentSourceKind::from_str(&row.source_kind)?,
                sha1: row.sha1,
                size: u64::try_from(row.size).unwrap_or_default(),
                project_id: row.project_id,
                version_id: row.version_id,
                pending: row
                    .pending
                    .as_deref()
                    .and_then(PendingChange::from_str),
            },
            EntrySides {
                enabled: row.enabled,
                server_requirement: ContentRequirement::from_str(
                    &row.server_requirement,
                )?,
                client_requirement: ContentRequirement::from_str(
                    &row.client_requirement,
                )?,
                unlocked: row.sides_unlocked,
                server_override: row.server_override,
            },
        ))
    }
}

fn file_name(relative_path: &str) -> &str {
    relative_path.rsplit('/').next().unwrap_or(relative_path)
}

fn signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// Every file the engine tracks on the server, with its sides, by path. The pack's client-only
/// files are not among them.
pub(crate) async fn tracked_files(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<Vec<(TrackedFile, EntrySides)>> {
    let rows: Vec<TrackedRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{TRACKED} ORDER BY f.relative_path"
    )))
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

pub(crate) async fn tracked_file(
    pool: &SqlitePool,
    instance_id: &str,
    relative_path: &str,
) -> crate::Result<Option<(TrackedFile, EntrySides)>> {
    let row: Option<TrackedRow> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "{TRACKED} AND f.relative_path = ?"
    )))
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(pool)
    .await?;
    row.map(TryInto::try_into).transpose()
}

async fn remove_file_rows(
    tx: &mut SqliteConnection,
    instance_id: &str,
    relative_path: &str,
    missing: bool,
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM instance_content_entries WHERE file_id IN (SELECT id FROM instance_files \
         WHERE instance_id = ? AND relative_path = ? AND missing = ?)",
    )
    .bind(instance_id)
    .bind(relative_path)
    .bind(missing)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM instance_files WHERE instance_id = ? AND relative_path = ? AND missing = ?",
    )
    .bind(instance_id)
    .bind(relative_path)
    .bind(missing)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Record a file the engine placed or found, replacing what was recorded for its path but its
/// sides, which stay with the path. A client-only row of the pack at the path gives way to it.
pub(crate) async fn upsert_tracked_file(
    pool: &SqlitePool,
    instance_id: &str,
    content_set_id: &str,
    file: &TrackedFile,
) -> crate::Result<()> {
    let now = Utc::now().timestamp();
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    remove_file_rows(&mut tx, instance_id, &file.relative_path, true).await?;
    let file_id: String = sqlx::query_scalar(
        "INSERT INTO instance_files (id, instance_id, relative_path, file_name, enabled, sha1, \
         size, missing, pending, added_at, modified_at) VALUES (?, ?, ?, ?, 1, ?, ?, 0, ?, ?, ?) \
         ON CONFLICT (instance_id, relative_path) DO UPDATE SET sha1 = excluded.sha1, \
         size = excluded.size, pending = excluded.pending, modified_at = excluded.modified_at \
         RETURNING id",
    )
    .bind(format!("instance-file:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(&file.relative_path)
    .bind(file_name(&file.relative_path))
    .bind(&file.sha1)
    .bind(signed(file.size))
    .bind(file.pending.map(PendingChange::as_str))
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "UPDATE instance_content_entries SET content_set_id = ? WHERE file_id = ?",
    )
    .bind(content_set_id)
    .bind(&file_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO instance_content_entries (id, instance_id, content_set_id, file_id, \
         project_type, project_id, version_id, source_kind, server_requirement, \
         client_requirement, enabled, added_at, modified_at) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'required', 'required', 1, ?, ?) \
         ON CONFLICT (content_set_id, file_id) WHERE file_id IS NOT NULL DO UPDATE SET \
         project_type = excluded.project_type, project_id = excluded.project_id, \
         version_id = excluded.version_id, source_kind = excluded.source_kind, \
         modified_at = excluded.modified_at",
    )
    .bind(format!("content-entry:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(content_set_id)
    .bind(&file_id)
    .bind(project_type_of(&file.relative_path).map(|kind| kind.get_name()))
    .bind(&file.project_id)
    .bind(&file.version_id)
    .bind(file.source_kind.as_str())
    .bind(now)
    .bind(now)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

/// Forget a tracked file, its sides with it.
pub(crate) async fn remove_tracked_file(
    pool: &SqlitePool,
    instance_id: &str,
    relative_path: &str,
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    remove_file_rows(&mut tx, instance_id, relative_path, false).await?;
    tx.commit().await?;
    Ok(())
}

/// Record a tracked file's sides. `false` when the engine does not track the file.
pub(crate) async fn set_entry_sides(
    pool: &SqlitePool,
    instance_id: &str,
    relative_path: &str,
    sides: &EntrySides,
) -> crate::Result<bool> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    let file_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM instance_files WHERE instance_id = ? AND relative_path = ? AND missing = 0",
    )
    .bind(instance_id)
    .bind(relative_path)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(file_id) = file_id else {
        return Ok(false);
    };
    let now = Utc::now().timestamp();
    sqlx::query(
        "UPDATE instance_content_entries SET enabled = ?, server_requirement = ?, \
         client_requirement = ?, sides_unlocked = ?, server_override = ?, modified_at = ? \
         WHERE file_id = ?",
    )
    .bind(sides.enabled)
    .bind(sides.server_requirement.as_str())
    .bind(sides.client_requirement.as_str())
    .bind(sides.unlocked)
    .bind(sides.server_override)
    .bind(now)
    .bind(&file_id)
    .execute(&mut *tx)
    .await?;
    // The file runs on the server unless it is off for both or for the server alone.
    sqlx::query("UPDATE instance_files SET enabled = ? WHERE id = ?")
        .bind(
            sides.enabled
                && sides.server_requirement != ContentRequirement::Unsupported,
        )
        .bind(&file_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(true)
}

/// Record the client-only files of the pack the server follows, in place of those recorded
/// before. A path the engine already tracks keeps its row.
pub(crate) async fn set_skipped_files(
    pool: &SqlitePool,
    instance_id: &str,
    content_set_id: &str,
    skipped: &[SkippedFile],
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query(
        "DELETE FROM instance_content_entries WHERE file_id IN (SELECT id FROM instance_files \
         WHERE instance_id = ? AND missing = 1)",
    )
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "DELETE FROM instance_files WHERE instance_id = ? AND missing = 1",
    )
    .bind(instance_id)
    .execute(&mut *tx)
    .await?;
    for file in skipped {
        insert_skipped_file(&mut tx, instance_id, content_set_id, file).await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Record one client-only file of the pack, unless the engine tracks its path.
async fn insert_skipped_file(
    tx: &mut SqliteConnection,
    instance_id: &str,
    content_set_id: &str,
    file: &SkippedFile,
) -> crate::Result<()> {
    let now = Utc::now().timestamp();
    let file_id: Option<String> = sqlx::query_scalar(
        "INSERT OR IGNORE INTO instance_files (id, instance_id, relative_path, file_name, \
         enabled, sha1, size, missing, added_at, modified_at) \
         VALUES (?, ?, ?, ?, 0, ?, ?, 1, ?, ?) RETURNING id",
    )
    .bind(format!("instance-file:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(&file.relative_path)
    .bind(file_name(&file.relative_path))
    .bind(&file.sha1)
    .bind(signed(file.size))
    .bind(now)
    .bind(now)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(file_id) = file_id else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO instance_content_entries (id, instance_id, content_set_id, file_id, \
         project_type, project_id, version_id, source_kind, server_requirement, \
         client_requirement, enabled, added_at, modified_at) \
         SELECT ?, ?, id, ?, ?, ?, ?, source_kind, 'unsupported', 'required', 1, ?, ? \
         FROM instance_content_sets WHERE id = ?",
    )
    .bind(format!("content-entry:{}", Uuid::new_v4()))
    .bind(instance_id)
    .bind(&file_id)
    .bind(project_type_of(&file.relative_path).map(|kind| kind.get_name()))
    .bind(&file.project_id)
    .bind(&file.version_id)
    .bind(now)
    .bind(now)
    .bind(content_set_id)
    .execute(&mut *tx)
    .await?;
    Ok(())
}

/// Forget the tracked file at `file`'s path and record it as a client-only file of the pack
/// again: someone turned the server side of an [`EntrySides::server_override`] file off.
pub(crate) async fn return_to_client_only(
    pool: &SqlitePool,
    instance_id: &str,
    content_set_id: &str,
    file: &SkippedFile,
) -> crate::Result<()> {
    let mut tx = pool.begin_with("BEGIN IMMEDIATE").await?;
    remove_file_rows(&mut tx, instance_id, &file.relative_path, false).await?;
    insert_skipped_file(&mut tx, instance_id, content_set_id, file).await?;
    tx.commit().await?;
    Ok(())
}

/// The pack's client-only files someone turned on for the server.
pub(crate) async fn server_overrides(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<ServerOn> {
    let rows: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT f.relative_path, e.project_id FROM instance_files f \
         JOIN instance_content_entries e ON e.file_id = f.id \
         WHERE f.instance_id = ? AND f.missing = 0 AND e.server_override = 1 \
         AND e.source_kind != 'local'",
    )
    .bind(instance_id)
    .fetch_all(pool)
    .await?;
    let mut on = ServerOn::default();
    for (path, project_id) in rows {
        on.paths.insert(path);
        on.projects.extend(project_id);
    }
    Ok(on)
}

/// Mark the pack files at `paths` as turned on for the server by hand
/// ([`EntrySides::server_override`]): a pack update placed them only because they were.
pub(crate) async fn set_server_overrides(
    pool: &SqlitePool,
    instance_id: &str,
    paths: &HashSet<String>,
) -> crate::Result<()> {
    for path in paths {
        sqlx::query(
            "UPDATE instance_content_entries SET server_override = 1, sides_unlocked = 1 \
             WHERE file_id = (SELECT id FROM instance_files \
             WHERE instance_id = ? AND relative_path = ? AND missing = 0)",
        )
        .bind(instance_id)
        .bind(path)
        .execute(pool)
        .await?;
    }
    Ok(())
}

/// The client-only files of the pack the server follows, by path.
pub(crate) async fn skipped_files(
    pool: &SqlitePool,
    instance_id: &str,
) -> crate::Result<Vec<SkippedFile>> {
    let rows: Vec<(String, String, i64, Option<String>, Option<String>)> =
        sqlx::query_as(
            "SELECT f.relative_path, f.sha1, f.size, e.project_id, e.version_id \
             FROM instance_files f JOIN instance_content_entries e ON e.file_id = f.id \
             WHERE f.instance_id = ? AND f.missing = 1 ORDER BY f.relative_path",
        )
        .bind(instance_id)
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|(relative_path, sha1, size, project_id, version_id)| {
            SkippedFile {
                relative_path,
                sha1,
                size: u64::try_from(size).unwrap_or_default(),
                project_id,
                version_id,
            }
        })
        .collect())
}
