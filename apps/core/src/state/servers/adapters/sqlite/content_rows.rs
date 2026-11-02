//! The rows of a server's content tables (`server_content_sets`, `server_files`,
//! `server_content_entries`) and of `pending_packs`, and the model they read into. Mirrors
//! app-lib's `state/instances/adapters/sqlite/content_rows.rs`: a row struct per table read with
//! `TryFrom` into the model, and the queries beside them, with `server` for `instance`. Copal's
//! differences: one content set per server (the pack it follows), every entry is one file's,
//! and a pack's client-only files are `missing` rows (see the schema in `001_init.sql`).

use std::collections::HashMap;

use bedrock::state::{ContentRequirement, ModLoader};
use chrono::{DateTime, TimeZone, Utc};
use sqlx::{SqliteExecutor, SqlitePool};

use crate::{
    access::sync::owe,
    api::backend::{ForgetPlayers, OwedCall},
    state::servers::model::{
        ContentSet, ContentSetStatus, ContentSourceKind, EntrySides, Pack,
        Pending, PendingPack, SkippedFile, TrackedFile, project_type_of,
    },
};

/// A tracked file: its `server_files` row joined with its entry.
#[derive(Debug, sqlx::FromRow)]
struct TrackedFileRow {
    relative_path: String,
    sha1: String,
    size: i64,
    source_kind: String,
    project_id: Option<String>,
    version_id: Option<String>,
    pending: Option<String>,
    enabled: bool,
    server_requirement: String,
    client_requirement: String,
    sides_unlocked: bool,
}

const TRACKED_COLUMNS: &str = "f.relative_path, f.sha1, f.size, e.source_kind, e.project_id, \
     e.version_id, e.pending, e.enabled, e.server_requirement, e.client_requirement, \
     e.sides_unlocked";

impl TryFrom<TrackedFileRow> for (TrackedFile, EntrySides) {
    type Error = sqlx::Error;

    fn try_from(row: TrackedFileRow) -> sqlx::Result<Self> {
        Ok((
            TrackedFile {
                relative_path: row.relative_path,
                source_kind: source_kind(&row.source_kind)?,
                sha1: row.sha1,
                size: unsigned(row.size),
                project_id: row.project_id,
                version_id: row.version_id,
                pending: row.pending.as_deref().and_then(Pending::from_str),
            },
            EntrySides {
                enabled: row.enabled,
                server_requirement: requirement(&row.server_requirement)?,
                client_requirement: requirement(&row.client_requirement)?,
                unlocked: row.sides_unlocked,
            },
        ))
    }
}

/// A row of `server_content_sets`.
#[derive(Debug, sqlx::FromRow)]
struct ContentSetRow {
    id: String,
    server_id: String,
    name: String,
    source_kind: String,
    status: String,
    game_version: String,
    protocol_version: Option<i64>,
    loader: String,
    loader_version: Option<String>,
    created: i64,
    modified: i64,
    project_id: Option<String>,
    version_id: Option<String>,
    version_number: String,
    summary: Option<String>,
    sha1: String,
    instance_id: Option<String>,
    instance_name: Option<String>,
    shared_instance_id: Option<String>,
    shared_version: Option<i64>,
    error: Option<String>,
}

impl TryFrom<ContentSetRow> for Pack {
    type Error = sqlx::Error;

    fn try_from(row: ContentSetRow) -> sqlx::Result<Self> {
        Ok(Self {
            set: ContentSet {
                id: row.id,
                instance_id: row.server_id,
                name: row.name,
                source_kind: source_kind(&row.source_kind)?,
                status: ContentSetStatus::from_str(&row.status)
                    .map_err(decode)?,
                game_version: row.game_version,
                protocol_version: row
                    .protocol_version
                    .and_then(|version| u32::try_from(version).ok()),
                loader: ModLoader::from_string(&row.loader),
                loader_version: row.loader_version,
                created: timestamp(row.created),
                modified: timestamp(row.modified),
            },
            project_id: row.project_id,
            version_id: row.version_id,
            version_number: row.version_number,
            summary: row.summary,
            sha1: row.sha1,
            instance_id: row.instance_id,
            instance_name: row.instance_name,
            shared_instance_id: row.shared_instance_id,
            shared_version: row.shared_version,
            error: row.error,
        })
    }
}

/// A `missing` row: a client-only file of the pack.
#[derive(Debug, sqlx::FromRow)]
struct SkippedFileRow {
    relative_path: String,
    sha1: String,
    size: i64,
    project_id: Option<String>,
    version_id: Option<String>,
}

impl From<SkippedFileRow> for SkippedFile {
    fn from(row: SkippedFileRow) -> Self {
        Self {
            relative_path: row.relative_path,
            sha1: row.sha1,
            size: unsigned(row.size),
            project_id: row.project_id,
            version_id: row.version_id,
        }
    }
}

fn decode(error: bedrock::Error) -> sqlx::Error {
    sqlx::Error::Decode(error.to_string().into())
}

fn source_kind(value: &str) -> sqlx::Result<ContentSourceKind> {
    ContentSourceKind::from_str(value).map_err(decode)
}

fn requirement(value: &str) -> sqlx::Result<ContentRequirement> {
    ContentRequirement::from_str(value).map_err(decode)
}

fn timestamp(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0).single().unwrap_or_default()
}

fn unsigned(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

fn signed(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

fn file_name(relative_path: &str) -> &str {
    relative_path.rsplit('/').next().unwrap_or(relative_path)
}

/// Every file Core tracks on the server, with its sides, by path. A pack's client-only files
/// are not among them.
pub async fn get_tracked_files_with_sides(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Vec<(TrackedFile, EntrySides)>> {
    let rows: Vec<TrackedFileRow> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {TRACKED_COLUMNS} FROM server_files f \
             JOIN server_content_entries e ON e.file_id = f.id \
             WHERE f.server_id = ? AND f.missing = 0 ORDER BY f.relative_path"
        )))
        .bind(server_id)
        .fetch_all(db)
        .await?;
    rows.into_iter().map(TryInto::try_into).collect()
}

/// Every file Core tracks on the server, by path.
pub async fn get_tracked_files(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Vec<TrackedFile>> {
    Ok(get_tracked_files_with_sides(db, server_id)
        .await?
        .into_iter()
        .map(|(file, _)| file)
        .collect())
}

/// The sides of every tracked file, by path.
pub async fn get_entry_sides(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<HashMap<String, EntrySides>> {
    Ok(get_tracked_files_with_sides(db, server_id)
        .await?
        .into_iter()
        .map(|(file, sides)| (file.relative_path, sides))
        .collect())
}

pub async fn get_tracked_file(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    relative_path: &str,
) -> sqlx::Result<Option<(TrackedFile, EntrySides)>> {
    let row: Option<TrackedFileRow> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {TRACKED_COLUMNS} FROM server_files f \
             JOIN server_content_entries e ON e.file_id = f.id \
             WHERE f.server_id = ? AND f.relative_path = ? AND f.missing = 0"
        )))
        .bind(server_id)
        .bind(relative_path)
        .fetch_optional(db)
        .await?;
    row.map(TryInto::try_into).transpose()
}

/// Record a file Core placed or found, replacing what was recorded for its path but its sides,
/// which stay with the path. A client-only row of the pack at the path gives way to it.
pub async fn upsert_tracked_file(
    db: &SqlitePool,
    server_id: &str,
    file: &TrackedFile,
) -> sqlx::Result<()> {
    let now = Utc::now().timestamp();
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query(
        "DELETE FROM server_files WHERE server_id = ? AND relative_path = ? AND missing = 1",
    )
    .bind(server_id)
    .bind(&file.relative_path)
    .execute(&mut *tx)
    .await?;
    let file_id: String = sqlx::query_scalar(
        "INSERT INTO server_files (id, server_id, relative_path, file_name, enabled, sha1, size, \
         missing, added_at, modified_at) VALUES (?, ?, ?, ?, 1, ?, ?, 0, ?, ?) \
         ON CONFLICT (server_id, relative_path) DO UPDATE SET sha1 = excluded.sha1, \
         size = excluded.size, modified_at = excluded.modified_at RETURNING id",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(server_id)
    .bind(&file.relative_path)
    .bind(file_name(&file.relative_path))
    .bind(&file.sha1)
    .bind(signed(file.size))
    .bind(now)
    .bind(now)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO server_content_entries (id, server_id, content_set_id, file_id, \
         project_type, project_id, version_id, source_kind, added_at, modified_at, pending) \
         VALUES (?, ?, (SELECT id FROM server_content_sets WHERE server_id = ? AND ? != 'local'), \
         ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (file_id) DO UPDATE SET content_set_id = excluded.content_set_id, \
         project_type = excluded.project_type, project_id = excluded.project_id, \
         version_id = excluded.version_id, source_kind = excluded.source_kind, \
         modified_at = excluded.modified_at, pending = excluded.pending",
    )
    .bind(uuid::Uuid::new_v4().to_string())
    .bind(server_id)
    .bind(server_id)
    .bind(file.source_kind.as_str())
    .bind(&file_id)
    .bind(project_type_of(&file.relative_path).map(|kind| kind.as_str()))
    .bind(&file.project_id)
    .bind(&file.version_id)
    .bind(file.source_kind.as_str())
    .bind(now)
    .bind(now)
    .bind(file.pending.map(Pending::as_str))
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Forget a tracked file, its sides with it.
pub async fn remove_tracked_file(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    relative_path: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "DELETE FROM server_files WHERE server_id = ? AND relative_path = ? AND missing = 0",
    )
    .bind(server_id)
    .bind(relative_path)
    .execute(db)
    .await?;
    Ok(())
}

/// Record the Modrinth project and version of a tracked file found by its hash.
pub async fn set_project(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    relative_path: &str,
    project_id: &str,
    version_id: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE server_content_entries SET project_id = ?, version_id = ? WHERE file_id = \
         (SELECT id FROM server_files WHERE server_id = ? AND relative_path = ? AND missing = 0)",
    )
    .bind(project_id)
    .bind(version_id)
    .bind(server_id)
    .bind(relative_path)
    .execute(db)
    .await?;
    Ok(())
}

/// Record a tracked file's sides. `false` when Core does not track the file.
pub async fn set_entry_sides(
    db: &SqlitePool,
    server_id: &str,
    relative_path: &str,
    sides: &EntrySides,
) -> sqlx::Result<bool> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let file_id: Option<String> = sqlx::query_scalar(
        "SELECT id FROM server_files WHERE server_id = ? AND relative_path = ? AND missing = 0",
    )
    .bind(server_id)
    .bind(relative_path)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(file_id) = file_id else {
        return Ok(false);
    };
    sqlx::query(
        "UPDATE server_content_entries SET enabled = ?, server_requirement = ?, \
         client_requirement = ?, sides_unlocked = ?, modified_at = ? WHERE file_id = ?",
    )
    .bind(sides.enabled)
    .bind(sides.server_requirement.as_str())
    .bind(sides.client_requirement.as_str())
    .bind(sides.unlocked)
    .bind(Utc::now().timestamp())
    .bind(&file_id)
    .execute(&mut *tx)
    .await?;
    // The file runs on the server unless it is off for both or for the server alone.
    sqlx::query("UPDATE server_files SET enabled = ? WHERE id = ?")
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
/// before. A path Core already tracks keeps its row.
pub async fn set_skipped_files(
    db: &SqlitePool,
    server_id: &str,
    skipped: &[SkippedFile],
) -> sqlx::Result<()> {
    let now = Utc::now().timestamp();
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    sqlx::query("DELETE FROM server_files WHERE server_id = ? AND missing = 1")
        .bind(server_id)
        .execute(&mut *tx)
        .await?;
    for file in skipped {
        let file_id: Option<String> = sqlx::query_scalar(
            "INSERT OR IGNORE INTO server_files (id, server_id, relative_path, file_name, \
             enabled, sha1, size, missing, added_at, modified_at) \
             VALUES (?, ?, ?, ?, 0, ?, ?, 1, ?, ?) RETURNING id",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(server_id)
        .bind(&file.relative_path)
        .bind(file_name(&file.relative_path))
        .bind(&file.sha1)
        .bind(signed(file.size))
        .bind(now)
        .bind(now)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(file_id) = file_id else { continue };
        sqlx::query(
            "INSERT INTO server_content_entries (id, server_id, content_set_id, file_id, \
             project_type, project_id, version_id, source_kind, server_requirement, \
             client_requirement, enabled, added_at, modified_at) \
             SELECT ?, ?, id, ?, ?, ?, ?, source_kind, 'unsupported', 'required', 1, ?, ? \
             FROM server_content_sets WHERE server_id = ?",
        )
        .bind(uuid::Uuid::new_v4().to_string())
        .bind(server_id)
        .bind(&file_id)
        .bind(project_type_of(&file.relative_path).map(|kind| kind.as_str()))
        .bind(&file.project_id)
        .bind(&file.version_id)
        .bind(now)
        .bind(now)
        .bind(server_id)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await
}

/// The client-only files of the pack the server follows, by path.
pub async fn get_skipped_files(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Vec<SkippedFile>> {
    let rows: Vec<SkippedFileRow> = sqlx::query_as(
        "SELECT f.relative_path, f.sha1, f.size, e.project_id, e.version_id \
         FROM server_files f JOIN server_content_entries e ON e.file_id = f.id \
         WHERE f.server_id = ? AND f.missing = 1 ORDER BY f.relative_path",
    )
    .bind(server_id)
    .fetch_all(db)
    .await?;
    Ok(rows.into_iter().map(Into::into).collect())
}

/// The pack the server follows.
pub async fn get_pack(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Option<Pack>> {
    let row: Option<ContentSetRow> =
        sqlx::query_as("SELECT * FROM server_content_sets WHERE server_id = ?")
            .bind(server_id)
            .fetch_optional(db)
            .await?;
    row.map(TryInto::try_into).transpose()
}

/// Record the pack the server now follows. Its content set keeps its id and creation time
/// across updates. Moving off the instance it followed, to a pack of its own or another
/// instance, drops that instance's players as [`unlink`] does (wake sync with `Access::owed`
/// after), and a pack of its own drops any version of it waiting to apply.
/// The transactions here read before they write. They begin IMMEDIATE so they take the write
/// lock first and wait for it; a deferred one fails at once with "database is locked" when another
/// connection wrote after its read.
pub async fn set_pack(
    db: &SqlitePool,
    server_id: &str,
    pack: &Pack,
) -> sqlx::Result<()> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let followed: Option<String> = sqlx::query_scalar(
        "SELECT shared_instance_id FROM server_content_sets WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    if followed.is_some() && followed != pack.shared_instance_id {
        forget_players(&mut tx, server_id, followed.clone()).await?;
    }
    if followed.is_some() && pack.shared_instance_id.is_none() {
        sqlx::query("DELETE FROM pending_packs WHERE server_id = ?")
            .bind(server_id)
            .execute(&mut *tx)
            .await?;
    }
    let set = &pack.set;
    sqlx::query(
        "INSERT INTO server_content_sets (id, server_id, name, source_kind, status, \
         game_version, protocol_version, loader, loader_version, created, modified, project_id, \
         version_id, version_number, summary, sha1, instance_id, instance_name, \
         shared_instance_id, shared_version, error) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (server_id) DO UPDATE SET name = excluded.name, \
         source_kind = excluded.source_kind, status = excluded.status, \
         game_version = excluded.game_version, protocol_version = excluded.protocol_version, \
         loader = excluded.loader, loader_version = excluded.loader_version, \
         modified = excluded.modified, project_id = excluded.project_id, \
         version_id = excluded.version_id, version_number = excluded.version_number, \
         summary = excluded.summary, sha1 = excluded.sha1, instance_id = excluded.instance_id, \
         instance_name = excluded.instance_name, \
         shared_instance_id = excluded.shared_instance_id, \
         shared_version = excluded.shared_version, error = excluded.error",
    )
    .bind(&set.id)
    .bind(server_id)
    .bind(&set.name)
    .bind(set.source_kind.as_str())
    .bind(set.status.as_str())
    .bind(&set.game_version)
    .bind(set.protocol_version.map(i64::from))
    .bind(set.loader.as_str())
    .bind(&set.loader_version)
    .bind(set.created.timestamp())
    .bind(set.modified.timestamp())
    .bind(&pack.project_id)
    .bind(&pack.version_id)
    .bind(&pack.version_number)
    .bind(&pack.summary)
    .bind(&pack.sha1)
    .bind(&pack.instance_id)
    .bind(&pack.instance_name)
    .bind(&pack.shared_instance_id)
    .bind(pack.shared_version)
    .bind(&pack.error)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Owe the backend dropping the server's players, who read `share_id`, the share of the instance
/// it followed, through it.
async fn forget_players(
    tx: &mut sqlx::SqliteConnection,
    server_id: &str,
    share_id: Option<String>,
) -> sqlx::Result<()> {
    owe(
        tx,
        &OwedCall::ForgetPlayers(ForgetPlayers {
            server_id: server_id.into(),
            share_id,
        }),
    )
    .await
}

/// Record why a pack install failed, or clear it with `None`. No pack, nothing to record.
pub async fn set_pack_error(
    db: &SqlitePool,
    server_id: &str,
    error: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE server_content_sets SET error = ? WHERE server_id = ?")
        .bind(error)
        .bind(server_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Stop following the pack, and drop any waiting to apply. Its files stay, as files someone
/// added; its client-only files are forgotten. A server that followed an instance owes the
/// backend dropping its players, who got that instance's share through it; wake sync with
/// `Access::owed` after.
pub async fn unlink(db: &SqlitePool, server_id: &str) -> sqlx::Result<()> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let followed: Option<String> = sqlx::query_scalar(
        "SELECT shared_instance_id FROM server_content_sets WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    if followed.is_some() {
        forget_players(&mut tx, server_id, followed).await?;
    }
    sqlx::query("DELETE FROM server_files WHERE server_id = ? AND missing = 1")
        .bind(server_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE server_content_entries SET source_kind = 'local', pending = NULL, \
         content_set_id = NULL WHERE server_id = ?",
    )
    .bind(server_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("DELETE FROM server_content_sets WHERE server_id = ?")
        .bind(server_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM pending_packs WHERE server_id = ?")
        .bind(server_id)
        .execute(&mut *tx)
        .await?;
    tx.commit().await
}

/// Whether any server's pack, or a pack waiting for its server to stop, is the `.mrpack` with
/// this SHA-1, so it must be kept.
pub async fn pack_in_use(db: &SqlitePool, sha1: &str) -> sqlx::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT (SELECT COUNT(*) FROM server_content_sets WHERE sha1 = ?1) \
         + (SELECT COUNT(*) FROM pending_packs WHERE sha1 = ?1)",
    )
    .bind(sha1)
    .fetch_one(db)
    .await?
        > 0)
}

/// Wait for the server to stop with this pack, in place of any that waited before. Returns the
/// SHA-1 of the pack it replaced, whose file may now be unused.
pub async fn set_pending(
    db: &SqlitePool,
    server_id: &str,
    pending: &PendingPack,
) -> sqlx::Result<Option<String>> {
    let mut tx = db.begin_with("BEGIN IMMEDIATE").await?;
    let replaced: Option<String> = sqlx::query_scalar(
        "SELECT sha1 FROM pending_packs WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT OR REPLACE INTO pending_packs (server_id, sha1, instance_id, instance_name, \
         shared_instance_id, shared_version, start_after, force) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(server_id)
    .bind(&pending.sha1)
    .bind(&pending.instance_id)
    .bind(&pending.instance_name)
    .bind(&pending.shared_instance_id)
    .bind(pending.shared_version)
    .bind(pending.start_after)
    .bind(pending.force)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(replaced.filter(|sha1| *sha1 != pending.sha1))
}

pub async fn get_pending(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Option<PendingPack>> {
    sqlx::query_as(
        "SELECT sha1, instance_id, instance_name, shared_instance_id, shared_version, \
         start_after, force FROM pending_packs WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await
}

/// Forget the waiting pack: it is being applied, or a newer push replaced it. Returns its SHA-1.
pub async fn clear_pending(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar(
        "DELETE FROM pending_packs WHERE server_id = ? RETURNING sha1",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await
}

/// The servers with a pack waiting.
pub async fn servers_pending(db: &SqlitePool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT server_id FROM pending_packs")
        .fetch_all(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(shared_instance_id: Option<&str>) -> Pack {
        Pack {
            set: ContentSet {
                id: uuid::Uuid::new_v4().to_string(),
                instance_id: "s1".into(),
                name: "Pack".into(),
                source_kind: if shared_instance_id.is_some() {
                    ContentSourceKind::SharedInstance
                } else {
                    ContentSourceKind::ImportedModpack
                },
                status: ContentSetStatus::Available,
                game_version: "1.21.1".into(),
                protocol_version: None,
                loader: ModLoader::Fabric,
                loader_version: None,
                created: Utc::now(),
                modified: Utc::now(),
            },
            project_id: None,
            version_id: None,
            version_number: "1".into(),
            summary: None,
            sha1: "a".repeat(40),
            instance_id: shared_instance_id.map(|_| "i".into()),
            instance_name: shared_instance_id.map(|_| "Instance".into()),
            shared_instance_id: shared_instance_id.map(Into::into),
            shared_version: shared_instance_id.map(|_| 1),
            error: None,
        }
    }

    fn tracked(path: &str, source_kind: ContentSourceKind) -> TrackedFile {
        TrackedFile {
            relative_path: path.into(),
            source_kind,
            sha1: "x".into(),
            size: 1,
            project_id: None,
            version_id: None,
            pending: None,
        }
    }

    async fn server() -> SqlitePool {
        let db = crate::db::memory().await;
        sqlx::query(
            "INSERT INTO servers (id, name, created_at, port, memory_mb) VALUES ('s1', 'A', 0, 25565, 1024)",
        )
        .execute(&db)
        .await
        .unwrap();
        db
    }

    async fn owed(db: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM backend_owed")
            .fetch_one(db)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn leaving_an_instance_drops_its_players() {
        let db = server().await;
        set_pack(&db, "s1", &pack(Some("share-a"))).await.unwrap();
        let id = get_pack(&db, "s1").await.unwrap().unwrap().set.id;
        set_pack(&db, "s1", &pack(Some("share-a"))).await.unwrap();
        assert_eq!(owed(&db).await, 0, "a new version of the same instance");
        assert_eq!(
            get_pack(&db, "s1").await.unwrap().unwrap().set.id,
            id,
            "the set keeps its id"
        );
        set_pack(&db, "s1", &pack(Some("share-b"))).await.unwrap();
        assert_eq!(owed(&db).await, 1, "another instance");

        sqlx::query(
            "INSERT INTO pending_packs (server_id, sha1, instance_id, instance_name, \
             shared_instance_id, shared_version, start_after, force) \
             VALUES ('s1', 'b', 'i', 'Instance', 'share-b', 2, 0, 0)",
        )
        .execute(&db)
        .await
        .unwrap();
        set_pack(&db, "s1", &pack(None)).await.unwrap();
        assert_eq!(owed(&db).await, 2, "a pack of its own");
        assert_eq!(get_pending(&db, "s1").await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_file_keeps_its_sides_when_its_record_changes() {
        let db = server().await;
        set_pack(&db, "s1", &pack(None)).await.unwrap();
        let file = tracked("mods/a.jar", ContentSourceKind::ImportedModpack);
        upsert_tracked_file(&db, "s1", &file).await.unwrap();
        let off_for_players = EntrySides {
            client_requirement: ContentRequirement::Unsupported,
            unlocked: true,
            ..EntrySides::default()
        };
        assert!(
            set_entry_sides(&db, "s1", "mods/a.jar", &off_for_players)
                .await
                .unwrap()
        );
        let edited = TrackedFile {
            pending: Some(Pending::Changed),
            ..file
        };
        upsert_tracked_file(&db, "s1", &edited).await.unwrap();
        assert_eq!(
            get_tracked_file(&db, "s1", "mods/a.jar").await.unwrap(),
            Some((edited, off_for_players))
        );
        assert!(
            !set_entry_sides(&db, "s1", "mods/b.jar", &off_for_players)
                .await
                .unwrap(),
            "an untracked file has no sides to set"
        );
    }

    #[tokio::test]
    async fn client_only_files_are_listed_apart_and_go_with_the_pack() {
        let db = server().await;
        set_pack(&db, "s1", &pack(None)).await.unwrap();
        let skipped = SkippedFile {
            relative_path: "mods/client.jar".into(),
            sha1: "c".into(),
            size: 2,
            project_id: Some("p".into()),
            version_id: Some("v".into()),
        };
        set_skipped_files(&db, "s1", std::slice::from_ref(&skipped))
            .await
            .unwrap();
        assert_eq!(get_skipped_files(&db, "s1").await.unwrap(), [skipped]);
        assert!(get_tracked_files(&db, "s1").await.unwrap().is_empty());

        // A file placed at the path takes it over.
        let placed = tracked("mods/client.jar", ContentSourceKind::Local);
        upsert_tracked_file(&db, "s1", &placed).await.unwrap();
        assert!(get_skipped_files(&db, "s1").await.unwrap().is_empty());

        unlink(&db, "s1").await.unwrap();
        assert_eq!(get_tracked_files(&db, "s1").await.unwrap(), [placed]);
        assert!(get_pack(&db, "s1").await.unwrap().is_none());
    }
}
