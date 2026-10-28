//! `content_files` and `packs`: where each tracked file came from, and the pack a server follows.
//! The files themselves are the truth; a row only says who put a file there and what it was then.

use sqlx::{SqliteExecutor, SqlitePool};

/// Who put a tracked file there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum Origin {
    /// The pack the server follows.
    Pack,
    /// Someone on the server: a Modrinth project they added, or a file they uploaded.
    Added,
}

/// What a pack update wants for a file it found edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum Pending {
    /// The new pack has another version of the file.
    Changed,
    /// The new pack no longer has the file.
    Removed,
}

/// A row of `content_files`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct ContentFile {
    pub path: String,
    pub origin: Origin,
    /// As Core placed it.
    pub sha1: String,
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub pending: Option<Pending>,
}

const FILE_COLUMNS: &str =
    "path, origin, sha1, project_id, version_id, pending";

pub async fn files(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Vec<ContentFile>> {
    sqlx::query_as(&format!(
        "SELECT {FILE_COLUMNS} FROM content_files WHERE server_id = ? ORDER BY path"
    ))
    .bind(server_id)
    .fetch_all(db)
    .await
}

pub async fn file(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    path: &str,
) -> sqlx::Result<Option<ContentFile>> {
    sqlx::query_as(&format!(
        "SELECT {FILE_COLUMNS} FROM content_files WHERE server_id = ? AND path = ?"
    ))
    .bind(server_id)
    .bind(path)
    .fetch_optional(db)
    .await
}

/// Record a file Core placed, replacing what was recorded for its path.
pub async fn put(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    file: &ContentFile,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO content_files (server_id, path, origin, sha1, project_id, version_id, pending) \
         VALUES (?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (server_id, path) DO UPDATE SET origin = excluded.origin, \
         sha1 = excluded.sha1, project_id = excluded.project_id, \
         version_id = excluded.version_id, pending = excluded.pending",
    )
    .bind(server_id)
    .bind(&file.path)
    .bind(file.origin)
    .bind(&file.sha1)
    .bind(&file.project_id)
    .bind(&file.version_id)
    .bind(file.pending)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    path: &str,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM content_files WHERE server_id = ? AND path = ?")
        .bind(server_id)
        .bind(path)
        .execute(db)
        .await?;
    Ok(())
}

/// Record the Modrinth project and version of a file found by its hash.
pub async fn set_project(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    path: &str,
    project_id: &str,
    version_id: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE content_files SET project_id = ?, version_id = ? WHERE server_id = ? AND path = ?",
    )
    .bind(project_id)
    .bind(version_id)
    .bind(server_id)
    .bind(path)
    .execute(db)
    .await?;
    Ok(())
}

/// A row of `packs`.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct Pack {
    pub project_id: Option<String>,
    pub version_id: Option<String>,
    pub name: String,
    pub version_number: String,
    pub summary: Option<String>,
    pub sha1: String,
    pub instance_id: Option<String>,
    pub instance_name: Option<String>,
    /// The share and version the pack is, for a pack from an App instance (`content::shared`).
    pub shared_instance_id: Option<String>,
    pub shared_version: Option<i64>,
    pub error: Option<String>,
}

pub async fn pack(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Option<Pack>> {
    sqlx::query_as(
        "SELECT project_id, version_id, name, version_number, summary, sha1, instance_id, \
         instance_name, shared_instance_id, shared_version, error FROM packs          WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await
}

pub async fn set_pack(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    pack: &Pack,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO packs (server_id, project_id, version_id, name, version_number, summary, \
         sha1, instance_id, instance_name, shared_instance_id, shared_version, error) \
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?) \
         ON CONFLICT (server_id) DO UPDATE SET project_id = excluded.project_id, \
         version_id = excluded.version_id, name = excluded.name, \
         version_number = excluded.version_number, summary = excluded.summary, \
         sha1 = excluded.sha1, instance_id = excluded.instance_id, \
         instance_name = excluded.instance_name, \
         shared_instance_id = excluded.shared_instance_id, \
         shared_version = excluded.shared_version, error = excluded.error",
    )
    .bind(server_id)
    .bind(&pack.project_id)
    .bind(&pack.version_id)
    .bind(&pack.name)
    .bind(&pack.version_number)
    .bind(&pack.summary)
    .bind(&pack.sha1)
    .bind(&pack.instance_id)
    .bind(&pack.instance_name)
    .bind(&pack.shared_instance_id)
    .bind(pack.shared_version)
    .bind(&pack.error)
    .execute(db)
    .await?;
    Ok(())
}

/// Record why a pack install failed, or clear it with `None`. No row, nothing to record.
pub async fn set_pack_error(
    db: &SqlitePool,
    server_id: &str,
    error: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE packs SET error = ? WHERE server_id = ?")
        .bind(error)
        .bind(server_id)
        .execute(db)
        .await?;
    Ok(())
}

/// Stop following the pack, and drop any waiting to apply. Its files stay, as files someone
/// added.
pub async fn unlink(db: &SqlitePool, server_id: &str) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    sqlx::query("DELETE FROM packs WHERE server_id = ?")
        .bind(server_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM pending_packs WHERE server_id = ?")
        .bind(server_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE content_files SET origin = 'added', pending = NULL WHERE server_id = ?",
    )
    .bind(server_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await
}

/// Whether any server's pack, or a pack waiting for its server to stop, is the `.mrpack` with
/// this SHA-1, so it must be kept.
pub async fn pack_in_use(db: &SqlitePool, sha1: &str) -> sqlx::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT (SELECT COUNT(*) FROM packs WHERE sha1 = ?1) \
         + (SELECT COUNT(*) FROM pending_packs WHERE sha1 = ?1)",
    )
    .bind(sha1)
    .fetch_one(db)
    .await?
        > 0)
}

/// A row of `pending_packs`: a share version pushed while its server ran.
#[derive(Debug, Clone, PartialEq, Eq, sqlx::FromRow)]
pub struct PendingPack {
    pub sha1: String,
    pub instance_id: String,
    pub instance_name: String,
    pub shared_instance_id: String,
    pub shared_version: i64,
    pub start_after: bool,
    pub force: bool,
}

/// Wait for the server to stop with this pack, in place of any that waited before. Returns the
/// SHA-1 of the pack it replaced, whose file may now be unused.
pub async fn set_pending(
    db: &SqlitePool,
    server_id: &str,
    pending: &PendingPack,
) -> sqlx::Result<Option<String>> {
    let mut tx = db.begin().await?;
    let replaced = pending_sha1(&mut *tx, server_id).await?;
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

async fn pending_sha1(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar("SELECT sha1 FROM pending_packs WHERE server_id = ?")
        .bind(server_id)
        .fetch_optional(db)
        .await
}

pub async fn pending(
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
