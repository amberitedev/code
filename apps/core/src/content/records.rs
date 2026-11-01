//! `content_files` and `packs`: where each tracked file came from, and the pack a server follows.
//! The files themselves are the truth; a row only says who put a file there and what it was then.

use sqlx::{SqliteExecutor, SqlitePool};

use crate::{
    access::sync::owe,
    api::backend::{ForgetPlayers, OwedCall},
};

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
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {FILE_COLUMNS} FROM content_files WHERE server_id = ? ORDER BY path"
    )))
    .bind(server_id)
    .fetch_all(db)
    .await
}

pub async fn file(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    path: &str,
) -> sqlx::Result<Option<ContentFile>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {FILE_COLUMNS} FROM content_files WHERE server_id = ? AND path = ?"
    )))
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

/// A row of `side_toggles`: which sides an addon is on for, by its enabled path. A file with no
/// row is on for both.
#[derive(Debug, Clone, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct SideToggle {
    pub path: String,
    /// The file is `.disabled` on disk, but the server's share still carries it.
    pub disabled_server: bool,
    /// The server runs it; its share leaves it out.
    pub disabled_player: bool,
    pub unlocked: bool,
}

pub async fn toggles(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Vec<SideToggle>> {
    sqlx::query_as(
        "SELECT path, disabled_server, disabled_player, unlocked FROM side_toggles \
         WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_all(db)
    .await
}

/// Record a file's toggles. A file on for both sides and never set by hand keeps no row.
pub async fn set_toggle(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    toggle: &SideToggle,
) -> sqlx::Result<()> {
    if *toggle
        == (SideToggle {
            path: toggle.path.clone(),
            ..SideToggle::default()
        })
    {
        return remove_toggle(db, server_id, &toggle.path).await;
    }
    sqlx::query(
        "INSERT INTO side_toggles (server_id, path, disabled_server, disabled_player, unlocked) \
         VALUES (?, ?, ?, ?, ?) ON CONFLICT (server_id, path) DO UPDATE SET \
         disabled_server = excluded.disabled_server, \
         disabled_player = excluded.disabled_player, unlocked = excluded.unlocked",
    )
    .bind(server_id)
    .bind(&toggle.path)
    .bind(toggle.disabled_server)
    .bind(toggle.disabled_player)
    .bind(toggle.unlocked)
    .execute(db)
    .await?;
    Ok(())
}

/// Give a file's toggles to the file replacing it, as an update does.
pub async fn move_toggle(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    from: &str,
    to: &str,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE OR REPLACE side_toggles SET path = ? WHERE server_id = ? AND path = ?",
    )
    .bind(to)
    .bind(server_id)
    .bind(from)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn remove_toggle(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    path: &str,
) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM side_toggles WHERE server_id = ? AND path = ?")
        .bind(server_id)
        .bind(path)
        .execute(db)
        .await?;
    Ok(())
}

/// The share of a server that follows no instance, once it is shared.
pub async fn share_id(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
) -> sqlx::Result<Option<String>> {
    Ok(sqlx::query_scalar::<_, Option<String>>(
        "SELECT share_id FROM servers WHERE id = ?",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await?
    .flatten())
}

pub async fn set_share_id(
    db: impl SqliteExecutor<'_>,
    server_id: &str,
    share_id: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE servers SET share_id = ? WHERE id = ?")
        .bind(share_id)
        .bind(server_id)
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

/// Record the pack the server now follows. Moving off the instance it followed, to a pack of its
/// own or another instance, drops that instance's players as [`unlink`] does (wake sync with
/// `Access::owed` after), and a pack of its own drops any version of it waiting to apply.
pub async fn set_pack(
    db: &SqlitePool,
    server_id: &str,
    pack: &Pack,
) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    let followed: Option<String> = sqlx::query_scalar(
        "SELECT shared_instance_id FROM packs WHERE server_id = ?",
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
/// added. A server that followed an instance owes the backend dropping its players, who got that
/// instance's share through it; wake sync with `Access::owed` after.
pub async fn unlink(db: &SqlitePool, server_id: &str) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    let followed: Option<String> = sqlx::query_scalar(
        "SELECT shared_instance_id FROM packs WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(&mut *tx)
    .await?
    .flatten();
    if followed.is_some() {
        forget_players(&mut tx, server_id, followed).await?;
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn pack(shared_instance_id: Option<&str>) -> Pack {
        Pack {
            project_id: None,
            version_id: None,
            name: "Pack".into(),
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

    async fn owed(db: &SqlitePool) -> i64 {
        sqlx::query_scalar("SELECT COUNT(*) FROM backend_owed")
            .fetch_one(db)
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn leaving_an_instance_drops_its_players() {
        let db = crate::db::memory().await;
        sqlx::query(
            "INSERT INTO servers (id, name, created_at, port, memory_mb) VALUES ('s1', 'A', 0, 25565, 1024)",
        )
        .execute(&db)
        .await
        .unwrap();
        set_pack(&db, "s1", &pack(Some("share-a"))).await.unwrap();
        set_pack(&db, "s1", &pack(Some("share-a"))).await.unwrap();
        assert_eq!(owed(&db).await, 0, "a new version of the same instance");
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
        assert_eq!(pending(&db, "s1").await.unwrap(), None);
    }
}
