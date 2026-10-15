//! Server rows and their run history in the database.

use sqlx::SqlitePool;

use super::shim::{EndedBy, ExitStatus};
use crate::network::firewall::SERVER_PORTS;

/// A row of `servers`.
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ServerRow {
    pub id: String,
    pub name: String,
    pub game_version: Option<String>,
    pub loader: Option<String>,
    pub loader_version: Option<String>,
    pub port: i64,
    pub memory_mb: i64,
    pub java_path: Option<String>,
    pub eula_accepted: bool,
}

/// How a run ended, as `runs.outcome` stores it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "TEXT", rename_all = "lowercase")]
pub enum Outcome {
    Stopped,
    Crashed,
    Killed,
}

/// The end of a run, as recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ended {
    pub at: i64,
    pub outcome: Outcome,
    pub exit_code: Option<i32>,
    /// The last console lines of a crashed run.
    pub last_lines: Option<String>,
}

/// Memory for a new server until the owner picks (step 8).
const DEFAULT_MEMORY_MB: i64 = 2048;

const COLUMNS: &str = "id, name, game_version, loader, loader_version, port, memory_mb, \
                       java_path, eula_accepted";

pub async fn list(db: &SqlitePool) -> sqlx::Result<Vec<ServerRow>> {
    sqlx::query_as(&format!(
        "SELECT {COLUMNS} FROM servers ORDER BY created_at, id"
    ))
    .fetch_all(db)
    .await
}

pub async fn get(db: &SqlitePool, id: &str) -> sqlx::Result<Option<ServerRow>> {
    sqlx::query_as(&format!("SELECT {COLUMNS} FROM servers WHERE id = ?"))
        .bind(id)
        .fetch_optional(db)
        .await
}

/// Create an unconfigured server on the lowest free port of the server range. `None` when
/// every port is taken.
pub async fn create(
    db: &SqlitePool,
    name: &str,
) -> sqlx::Result<Option<ServerRow>> {
    // Two creates at once can pick the same port; `port` is UNIQUE, so the later one picks again.
    loop {
        let used: Vec<i64> = sqlx::query_scalar("SELECT port FROM servers")
            .fetch_all(db)
            .await?;
        let Some(port) = SERVER_PORTS
            .map(i64::from)
            .find(|port| !used.contains(port))
        else {
            return Ok(None);
        };
        let id = uuid::Uuid::new_v4().to_string();
        let inserted = sqlx::query(
            "INSERT INTO servers (id, name, created_at, port, memory_mb) \
             VALUES (?, ?, unixepoch(), ?, ?)",
        )
        .bind(&id)
        .bind(name)
        .bind(port)
        .bind(DEFAULT_MEMORY_MB)
        .execute(db)
        .await;
        match inserted {
            Err(sqlx::Error::Database(error))
                if error.is_unique_violation() => {}
            inserted => {
                inserted?;
                return get(db, &id).await;
            }
        }
    }
}

/// Record what is installed in a server.
pub async fn set_installed(
    db: &SqlitePool,
    id: &str,
    loader: &str,
    game_version: &str,
    loader_version: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE servers SET loader = ?, game_version = ?, loader_version = ? WHERE id = ?",
    )
    .bind(loader)
    .bind(game_version)
    .bind(loader_version)
    .bind(id)
    .execute(db)
    .await?;
    Ok(())
}

pub async fn accept_eula(db: &SqlitePool, id: &str) -> sqlx::Result<bool> {
    let result =
        sqlx::query("UPDATE servers SET eula_accepted = 1 WHERE id = ?")
            .bind(id)
            .execute(db)
            .await?;
    Ok(result.rows_affected() == 1)
}

pub async fn start_run(
    db: &SqlitePool,
    server_id: &str,
    run_id: &str,
    started_at: i64,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO runs (id, server_id, started_at) VALUES (?, ?, ?)",
    )
    .bind(run_id)
    .bind(server_id)
    .bind(started_at)
    .execute(db)
    .await?;
    Ok(())
}

/// Runs of a server that have no end yet.
pub async fn open_runs(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT id FROM runs WHERE server_id = ? AND ended_at IS NULL ORDER BY started_at",
    )
    .bind(server_id)
    .fetch_all(db)
    .await
}

/// Servers with a run that has no end yet.
pub async fn servers_with_open_runs(
    db: &SqlitePool,
) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT DISTINCT server_id FROM runs WHERE ended_at IS NULL",
    )
    .fetch_all(db)
    .await
}

/// Record how a run ended. A run is ended once; later calls change nothing.
pub async fn end_run(
    db: &SqlitePool,
    run_id: &str,
    ended: &Ended,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE runs SET ended_at = ?, outcome = ?, exit_code = ?, last_lines = ? \
         WHERE id = ? AND ended_at IS NULL",
    )
    .bind(ended.at)
    .bind(ended.outcome)
    .bind(ended.exit_code)
    .bind(&ended.last_lines)
    .bind(run_id)
    .execute(db)
    .await?;
    Ok(())
}

/// How the server's last finished run ended.
pub async fn last_outcome(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Option<Outcome>> {
    sqlx::query_scalar(
        "SELECT outcome FROM runs WHERE server_id = ? AND ended_at IS NOT NULL \
         ORDER BY ended_at DESC, started_at DESC LIMIT 1",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await
}

/// How a run ended, from the status file the shim wrote for it. Without one the shim died
/// with Java, or the machine went down: a crash.
pub fn outcome(status: Option<&ExitStatus>) -> (Outcome, Option<i32>) {
    let Some(status) = status else {
        return (Outcome::Crashed, None);
    };
    let outcome = match (status.ended_by, status.exit_code) {
        (EndedBy::Kill, _) => Outcome::Killed,
        // Asked to stop, or stopped from the console or in game.
        (EndedBy::Stop, _) | (EndedBy::Exit, Some(0)) => Outcome::Stopped,
        (EndedBy::Exit, _) => Outcome::Crashed,
    };
    (outcome, status.exit_code)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{db, servers::shim::PROTOCOL};

    fn status(ended_by: EndedBy, exit_code: Option<i32>) -> ExitStatus {
        ExitStatus {
            v: PROTOCOL,
            run_id: "run".into(),
            exit_code,
            ended_at: 100,
            ended_by,
            error: None,
        }
    }

    #[test]
    fn outcomes_follow_who_ended_the_run() {
        assert_eq!(outcome(None), (Outcome::Crashed, None));
        assert_eq!(
            outcome(Some(&status(EndedBy::Exit, Some(0)))),
            (Outcome::Stopped, Some(0))
        );
        assert_eq!(
            outcome(Some(&status(EndedBy::Exit, Some(1)))),
            (Outcome::Crashed, Some(1))
        );
        assert_eq!(
            outcome(Some(&status(EndedBy::Stop, Some(130)))),
            (Outcome::Stopped, Some(130))
        );
        assert_eq!(
            outcome(Some(&status(EndedBy::Kill, Some(137)))),
            (Outcome::Killed, Some(137))
        );
        // Java never started.
        assert_eq!(
            outcome(Some(&status(EndedBy::Exit, None))),
            (Outcome::Crashed, None)
        );
    }

    #[tokio::test]
    async fn a_run_is_recorded_once_with_its_crash_lines() {
        let db = db::memory().await;
        let server = create(&db, "A").await.unwrap().unwrap();
        assert_eq!(server.port, i64::from(*SERVER_PORTS.start()));
        start_run(&db, &server.id, "run", 1).await.unwrap();
        assert_eq!(open_runs(&db, &server.id).await.unwrap(), ["run"]);
        assert_eq!(
            servers_with_open_runs(&db).await.unwrap(),
            [server.id.as_str()]
        );

        let crashed = Ended {
            at: 2,
            outcome: Outcome::Crashed,
            exit_code: Some(1),
            last_lines: Some("Exception in server tick loop".into()),
        };
        end_run(&db, "run", &crashed).await.unwrap();
        let stopped = Ended {
            outcome: Outcome::Stopped,
            ..crashed.clone()
        };
        end_run(&db, "run", &stopped).await.unwrap();

        let row: (i64, Outcome, Option<i32>, Option<String>) = sqlx::query_as(
            "SELECT ended_at, outcome, exit_code, last_lines FROM runs WHERE id = 'run'",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(
            row,
            (
                2,
                Outcome::Crashed,
                Some(1),
                Some("Exception in server tick loop".into())
            )
        );
        assert!(open_runs(&db, &server.id).await.unwrap().is_empty());
        assert_eq!(
            last_outcome(&db, &server.id).await.unwrap(),
            Some(Outcome::Crashed)
        );
    }

    #[tokio::test]
    async fn new_servers_take_the_next_free_port() {
        let db = db::memory().await;
        let first = create(&db, "A").await.unwrap().unwrap();
        let second = create(&db, "B").await.unwrap().unwrap();
        assert_eq!(second.port, first.port + 1);
        assert!(!second.eula_accepted);
        assert!(accept_eula(&db, &second.id).await.unwrap());
        assert!(get(&db, &second.id).await.unwrap().unwrap().eula_accepted);
    }
}
