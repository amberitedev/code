//! `backups`, `backup_operations`, and `backup_schedules`.

use sqlx::SqlitePool;

use crate::access::now;

/// Why a backup was made. Each kind keeps its own retention.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "snake_case")]
pub enum Kind {
    Manual,
    Scheduled,
    PreUpdate,
}

/// `backups.state`: where the backup's file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum State {
    Queued,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum OpKind {
    Create,
    Restore,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum OpState {
    Queued,
    Running,
    Done,
    Failed,
    Cancelled,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Backup {
    pub id: String,
    pub server_id: String,
    pub name: String,
    pub kind: Kind,
    pub state: State,
    pub created_at: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct Operation {
    pub id: i64,
    pub backup_id: String,
    pub server_id: String,
    pub kind: OpKind,
    pub state: OpState,
    pub user_id: Option<String>,
    pub queued_at: i64,
    pub started_at: Option<i64>,
    pub ended_at: Option<i64>,
    pub error: Option<String>,
    pub acked: bool,
}

const BACKUP: &str = "id, server_id, name, kind, state, created_at";
const OPERATION: &str = "id, backup_id, server_id, kind, state, user_id, queued_at, started_at, \
                         ended_at, error, acked";

/// Queue a new backup and the operation that creates it. Returns the operation's id.
pub async fn queue_backup(
    db: &SqlitePool,
    backup: &Backup,
    user_id: Option<&str>,
) -> sqlx::Result<i64> {
    let mut tx = db.begin().await?;
    sqlx::query(
        "INSERT INTO backups (id, server_id, name, kind, state, created_at) VALUES (?, ?, ?, ?, 'queued', ?)",
    )
    .bind(&backup.id)
    .bind(&backup.server_id)
    .bind(&backup.name)
    .bind(backup.kind)
    .bind(backup.created_at)
    .execute(&mut *tx)
    .await?;
    let id = queue(
        &mut tx,
        &backup.id,
        &backup.server_id,
        OpKind::Create,
        user_id,
    )
    .await?;
    tx.commit().await?;
    Ok(id)
}

/// Queue an operation on an existing backup.
pub async fn queue_operation(
    db: &SqlitePool,
    backup_id: &str,
    server_id: &str,
    kind: OpKind,
    user_id: Option<&str>,
) -> sqlx::Result<i64> {
    let mut tx = db.begin().await?;
    if kind == OpKind::Create {
        sqlx::query(
            "UPDATE backups SET state = 'queued', error = NULL WHERE id = ?",
        )
        .bind(backup_id)
        .execute(&mut *tx)
        .await?;
    }
    let id = queue(&mut tx, backup_id, server_id, kind, user_id).await?;
    tx.commit().await?;
    Ok(id)
}

async fn queue(
    tx: &mut sqlx::SqliteConnection,
    backup_id: &str,
    server_id: &str,
    kind: OpKind,
    user_id: Option<&str>,
) -> sqlx::Result<i64> {
    sqlx::query_scalar(
        "INSERT INTO backup_operations (backup_id, server_id, kind, state, user_id, queued_at) \
         VALUES (?, ?, ?, 'queued', ?, ?) RETURNING id",
    )
    .bind(backup_id)
    .bind(server_id)
    .bind(kind)
    .bind(user_id)
    .bind(now())
    .fetch_one(tx)
    .await
}

/// The oldest queued operation.
pub async fn next_queued(db: &SqlitePool) -> sqlx::Result<Option<Operation>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {OPERATION} FROM backup_operations WHERE state = 'queued' ORDER BY id LIMIT 1"
    )))
    .fetch_optional(db)
    .await
}

pub async fn operation(
    db: &SqlitePool,
    id: i64,
) -> sqlx::Result<Option<Operation>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {OPERATION} FROM backup_operations WHERE id = ?"
    )))
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn operations(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Vec<Operation>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {OPERATION} FROM backup_operations WHERE server_id = ? ORDER BY id"
    )))
    .bind(server_id)
    .fetch_all(db)
    .await
}

/// Claim a queued operation for running: mark it, and for a create its backup, as running.
/// `false` when it is no longer queued (cancelled meanwhile); it must not run then.
pub async fn start(
    db: &SqlitePool,
    operation: &Operation,
) -> sqlx::Result<bool> {
    let mut tx = db.begin().await?;
    let claimed = sqlx::query(
        "UPDATE backup_operations SET state = 'running', started_at = ?          WHERE id = ? AND state = 'queued'",
    )
    .bind(now())
    .bind(operation.id)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if !claimed {
        return Ok(false);
    }
    if operation.kind == OpKind::Create {
        sqlx::query("UPDATE backups SET state = 'running' WHERE id = ?")
            .bind(&operation.backup_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// Record how an operation ended. A create's backup becomes done with its size, or failed.
pub async fn finish(
    db: &SqlitePool,
    operation: &Operation,
    result: Result<u64, &str>,
) -> sqlx::Result<()> {
    let mut tx = db.begin().await?;
    let (state, error) = match result {
        Ok(_) => (OpState::Done, None),
        Err(error) => (OpState::Failed, Some(error)),
    };
    sqlx::query("UPDATE backup_operations SET state = ?, ended_at = ?, error = ? WHERE id = ?")
        .bind(state)
        .bind(now())
        .bind(error)
        .bind(operation.id)
        .execute(&mut *tx)
        .await?;
    if operation.kind == OpKind::Create {
        let (state, size) = match result {
            Ok(size) => (State::Done, i64::try_from(size).unwrap_or(i64::MAX)),
            Err(_) => (State::Failed, 0),
        };
        sqlx::query("UPDATE backups SET state = ?, size_bytes = ?, error = ? WHERE id = ?")
            .bind(state)
            .bind(size)
            .bind(error)
            .bind(&operation.backup_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await
}

/// Operations a Core that stopped left running end as failed.
pub async fn fail_interrupted(db: &SqlitePool) -> sqlx::Result<Vec<Operation>> {
    let interrupted: Vec<Operation> =
        sqlx::query_as(sqlx::AssertSqlSafe(format!(
            "SELECT {OPERATION} FROM backup_operations WHERE state = 'running'"
        )))
        .fetch_all(db)
        .await?;
    for operation in &interrupted {
        finish(db, operation, Err("Core stopped while this ran")).await?;
    }
    Ok(interrupted)
}

pub async fn backups(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Vec<Backup>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {BACKUP} FROM backups WHERE server_id = ? ORDER BY created_at DESC, rowid DESC"
    )))
    .bind(server_id)
    .fetch_all(db)
    .await
}

pub async fn backup(
    db: &SqlitePool,
    server_id: &str,
    id: &str,
) -> sqlx::Result<Option<Backup>> {
    sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {BACKUP} FROM backups WHERE server_id = ? AND id = ?"
    )))
    .bind(server_id)
    .bind(id)
    .fetch_optional(db)
    .await
}

pub async fn delete_backup(db: &SqlitePool, id: &str) -> sqlx::Result<()> {
    sqlx::query("DELETE FROM backups WHERE id = ?")
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
}

/// Whether a backup has an operation queued or running.
pub async fn busy(db: &SqlitePool, backup_id: &str) -> sqlx::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM backup_operations WHERE backup_id = ? AND state IN ('queued', 'running')",
    )
    .bind(backup_id)
    .fetch_one(db)
    .await?
        > 0)
}

/// Whether a server has an operation queued or running.
pub async fn server_busy(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM backup_operations WHERE server_id = ? AND state IN ('queued', 'running')",
    )
    .bind(server_id)
    .fetch_one(db)
    .await?
        > 0)
}

/// Done backups of `kind` past the newest `keep`, oldest last, that nothing is using.
pub async fn excess(
    db: &SqlitePool,
    server_id: &str,
    kind: Kind,
    keep: u32,
) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT id FROM backups WHERE server_id = ? AND kind = ? AND state = 'done' \
         AND id NOT IN (SELECT backup_id FROM backup_operations WHERE state IN ('queued', 'running')) \
         ORDER BY created_at DESC, rowid DESC LIMIT -1 OFFSET ?",
    )
    .bind(server_id)
    .bind(kind)
    .bind(i64::from(keep))
    .fetch_all(db)
    .await
}

/// Stop showing a finished operation.
pub async fn ack(
    db: &SqlitePool,
    server_id: &str,
    id: i64,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE backup_operations SET acked = 1 WHERE id = ? AND server_id = ? \
         AND state IN ('done', 'failed', 'cancelled')",
    )
    .bind(id)
    .bind(server_id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Cancel a queued operation, only while it is still queued: one the engine claimed runs. A
/// cancelled create takes its backup with it.
pub async fn cancel(
    db: &SqlitePool,
    server_id: &str,
    id: i64,
) -> sqlx::Result<bool> {
    let mut tx = db.begin().await?;
    let cancelled: Option<(String, OpKind)> = sqlx::query_as(
        "UPDATE backup_operations SET state = 'cancelled', ended_at = ?          WHERE id = ? AND server_id = ? AND state = 'queued' RETURNING backup_id, kind",
    )
    .bind(now())
    .bind(id)
    .bind(server_id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((backup_id, kind)) = cancelled else {
        return Ok(false);
    };
    if kind == OpKind::Create {
        sqlx::query("DELETE FROM backups WHERE id = ?")
            .bind(&backup_id)
            .execute(&mut *tx)
            .await?;
    }
    tx.commit().await?;
    Ok(true)
}

/// A server's schedule: hours between scheduled backups, how many to keep (its automatic slots;
/// 0 turns scheduled backups off), and when the schedule last came due with nothing new to back
/// up.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Schedule {
    pub interval_hours: u32,
    pub keep: u32,
    pub skipped_at: Option<i64>,
}

impl Default for Schedule {
    fn default() -> Self {
        Self {
            interval_hours: 24,
            keep: 7,
            skipped_at: None,
        }
    }
}

pub async fn schedule(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Schedule> {
    let row: Option<(i64, i64, Option<i64>)> = sqlx::query_as(
        "SELECT interval_hours, keep, skipped_at FROM backup_schedules WHERE server_id = ?",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await?;
    Ok(
        row.map_or_else(Schedule::default, |(interval, keep, skipped_at)| {
            Schedule {
                interval_hours: u32::try_from(interval).unwrap_or_default(),
                keep: u32::try_from(keep).unwrap_or_default(),
                skipped_at,
            }
        }),
    )
}

/// Set how often and how many; when the schedule last skipped stays.
pub async fn set_schedule(
    db: &SqlitePool,
    server_id: &str,
    schedule: Schedule,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO backup_schedules (server_id, interval_hours, keep) VALUES (?, ?, ?) \
         ON CONFLICT (server_id) DO UPDATE SET interval_hours = excluded.interval_hours, \
         keep = excluded.keep",
    )
    .bind(server_id)
    .bind(i64::from(schedule.interval_hours))
    .bind(i64::from(schedule.keep))
    .execute(db)
    .await?;
    Ok(())
}

/// Record that the schedule came due at `at` and found nothing new to back up.
pub async fn set_skipped(
    db: &SqlitePool,
    server_id: &str,
    at: i64,
) -> sqlx::Result<()> {
    let default = Schedule::default();
    sqlx::query(
        "INSERT INTO backup_schedules (server_id, interval_hours, keep, skipped_at) \
         VALUES (?, ?, ?, ?) ON CONFLICT (server_id) DO UPDATE SET skipped_at = excluded.skipped_at",
    )
    .bind(server_id)
    .bind(i64::from(default.interval_hours))
    .bind(i64::from(default.keep))
    .bind(at)
    .execute(db)
    .await?;
    Ok(())
}

/// The server's newest scheduled backup: how it stands, and when its last create was queued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastScheduled {
    pub backup_id: String,
    pub state: State,
    pub queued_at: i64,
}

pub async fn last_scheduled(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Option<LastScheduled>> {
    let row: Option<(String, State, i64)> = sqlx::query_as(
        "SELECT b.id, b.state, COALESCE(MAX(o.queued_at), b.created_at) FROM backups b \
         LEFT JOIN backup_operations o ON o.backup_id = b.id AND o.kind = 'create' \
         WHERE b.server_id = ? AND b.kind = 'scheduled' \
         GROUP BY b.id ORDER BY b.created_at DESC, b.rowid DESC LIMIT 1",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await?;
    Ok(row.map(|(backup_id, state, queued_at)| LastScheduled {
        backup_id,
        state,
        queued_at,
    }))
}

/// Rename a backup. `false` when the server has no such backup.
pub async fn rename(
    db: &SqlitePool,
    server_id: &str,
    id: &str,
    name: &str,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE backups SET name = ? WHERE server_id = ? AND id = ?",
    )
    .bind(name)
    .bind(server_id)
    .bind(id)
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// How many manual backups the server has or is making: the manual slots in use.
pub async fn manual_in_use(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<u32> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM backups WHERE server_id = ? AND kind = 'manual' \
         AND state IN ('queued', 'running', 'done')",
    )
    .bind(server_id)
    .fetch_one(db)
    .await?;
    Ok(u32::try_from(count).unwrap_or(u32::MAX))
}

/// When a run of the server last ended, or `None` if none has.
pub async fn last_run_end(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Option<i64>> {
    sqlx::query_scalar("SELECT MAX(ended_at) FROM runs WHERE server_id = ?")
        .bind(server_id)
        .fetch_one(db)
        .await
}
