//! Server rows and their run history in the database.

use std::{
    collections::HashMap,
    net::{IpAddr, Ipv4Addr, Ipv6Addr},
};

use color_eyre::eyre::{Result, eyre};
use engine::{Engine, state::InstanceMetadata};
use sqlx::SqlitePool;

use crate::network::firewall::SERVER_PORTS;
use crate::servers::shim::{EndedBy, ExitStatus};

/// A server: its row of `servers`, with the name, creation time and launch settings its engine
/// instance keeps. What it runs is its engine instance's too (see `servers::platform`).
#[derive(Debug, Clone)]
pub struct ServerRow {
    pub id: String,
    pub name: String,
    /// Unix seconds.
    pub created_at: i64,
    pub port: i64,
    pub memory_mb: i64,
    /// The server's own JVM flags, `None` for the tuned ones (`memory::TUNED_FLAGS`).
    pub jvm_flags: Option<Vec<String>>,
    /// Advanced override; `None` means the Java Core manages for this version.
    pub java_path: Option<String>,
    /// The managed Java major the owner chose; `None` means the one the Minecraft version needs.
    pub java_version: Option<i64>,
}

#[derive(sqlx::FromRow)]
struct Row {
    id: String,
    port: i64,
    java_version: Option<i64>,
}

impl ServerRow {
    fn new(row: Row, instance: InstanceMetadata) -> Self {
        let launch = instance.launch_overrides;
        Self {
            id: row.id,
            name: instance.instance.name,
            created_at: instance.instance.created.timestamp(),
            port: row.port,
            memory_mb: launch
                .memory
                .map_or(DEFAULT_MEMORY_MB, |memory| i64::from(memory.maximum)),
            jvm_flags: launch.extra_launch_args,
            java_path: launch.java_path,
            java_version: row.java_version,
        }
    }
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

/// Memory for a server whose owner has not picked any.
const DEFAULT_MEMORY_MB: i64 = 2048;

const COLUMNS: &str = "id, port, java_version";

/// Whether no other program on this machine holds `port`. Minecraft listens on the addresses
/// [`probed_ips`] gives, TCP for the game and UDP for query and voice mods, so Core tries to bind
/// the same. A port is taken when a bind fails because it is in use or reserved; a bind the OS
/// cannot do at all, such as IPv6 on a machine without it, says nothing about the port.
pub fn port_free(port: u16, local_only: Option<IpAddr>) -> bool {
    use std::{
        io::ErrorKind,
        net::{SocketAddr, TcpListener, UdpSocket},
    };

    // Windows Firewall prompts the developer for each test binary that listens beyond loopback.
    #[cfg(test)]
    assert!(
        local_only.is_some(),
        "tests open Servers with a loopback address"
    );

    probed_ips(local_only)
        .into_iter()
        .map(|ip| SocketAddr::new(ip, port))
        .flat_map(|address| {
            [
                TcpListener::bind(address).err(),
                UdpSocket::bind(address).err(),
            ]
        })
        .flatten()
        .all(|error| {
            !matches!(
                error.kind(),
                ErrorKind::AddrInUse | ErrorKind::PermissionDenied
            )
        })
}

/// Where a server listens: a local-only Core's own loopback address (see `Config::local_only`),
/// otherwise every interface, IPv4 and, where the machine has it, IPv6.
fn probed_ips(local_only: Option<IpAddr>) -> Vec<IpAddr> {
    match local_only {
        Some(ip) => vec![ip],
        None => vec![
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            IpAddr::V6(Ipv6Addr::UNSPECIFIED),
        ],
    }
}

fn engine_error(error: engine::Error) -> color_eyre::Report {
    eyre!("{error:#}")
}

/// Every server, oldest first.
pub async fn list(db: &SqlitePool, engine: &Engine) -> Result<Vec<ServerRow>> {
    let rows: Vec<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM servers"
    )))
    .fetch_all(db)
    .await?;
    let mut instances: HashMap<String, InstanceMetadata> =
        engine::instance::list(engine)
            .await
            .map_err(engine_error)?
            .into_iter()
            .map(|instance| (instance.instance.id.clone(), instance))
            .collect();
    let mut servers: Vec<ServerRow> = rows
        .into_iter()
        .filter_map(|row| {
            let instance = instances.remove(&row.id)?;
            Some(ServerRow::new(row, instance))
        })
        .collect();
    servers.sort_by(|a, b| (a.created_at, &a.id).cmp(&(b.created_at, &b.id)));
    Ok(servers)
}

/// The ids of every row, with or without an engine instance.
pub async fn ids(db: &SqlitePool) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar("SELECT id FROM servers")
        .fetch_all(db)
        .await
}

pub async fn get(
    db: &SqlitePool,
    engine: &Engine,
    id: &str,
) -> Result<Option<ServerRow>> {
    let row: Option<Row> = sqlx::query_as(sqlx::AssertSqlSafe(format!(
        "SELECT {COLUMNS} FROM servers WHERE id = ?"
    )))
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let instance = engine::instance::get(engine, id)
        .await
        .map_err(engine_error)?;
    Ok(instance.map(|instance| ServerRow::new(row, instance)))
}

/// Give the server `id`, whose engine instance exists, a row on the lowest port of the server
/// range that no server has and `free` accepts ([`port_free`] outside tests). Its port, or
/// `None` when every port is taken.
pub async fn create(
    db: &SqlitePool,
    id: &str,
    free: impl Fn(u16) -> bool,
) -> sqlx::Result<Option<i64>> {
    // Two creates at once can pick the same port; `port` is UNIQUE, so the later one picks again.
    loop {
        let used: Vec<i64> = sqlx::query_scalar("SELECT port FROM servers")
            .fetch_all(db)
            .await?;
        let Some(port) = SERVER_PORTS
            .filter(|port| !used.contains(&i64::from(*port)) && free(*port))
            .map(i64::from)
            .next()
        else {
            return Ok(None);
        };
        let inserted =
            sqlx::query("INSERT INTO servers (id, port) VALUES (?, ?)")
                .bind(id)
                .bind(port)
                .execute(db)
                .await;
        match inserted {
            Err(sqlx::Error::Database(error))
                if error.is_unique_violation() => {}
            inserted => {
                inserted?;
                return Ok(Some(port));
            }
        }
    }
}

/// Choose the managed Java major a server runs, `None` for the one its Minecraft version needs.
pub async fn set_java_version(
    db: &SqlitePool,
    id: &str,
    java_version: Option<i64>,
) -> sqlx::Result<()> {
    sqlx::query("UPDATE servers SET java_version = ? WHERE id = ?")
        .bind(java_version)
        .bind(id)
        .execute(db)
        .await?;
    Ok(())
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

/// Runs of a server that have no end yet and started before `started_before`.
pub async fn open_runs(
    db: &SqlitePool,
    server_id: &str,
    started_before: i64,
) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(
        "SELECT id FROM runs WHERE server_id = ? AND ended_at IS NULL AND started_at < ? \
         ORDER BY started_at",
    )
    .bind(server_id)
    .bind(started_before)
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

/// The server's last finished run.
pub async fn last_ended(
    db: &SqlitePool,
    server_id: &str,
) -> sqlx::Result<Option<String>> {
    sqlx::query_scalar(
        "SELECT id FROM runs WHERE server_id = ? AND ended_at IS NOT NULL \
         ORDER BY ended_at DESC, started_at DESC LIMIT 1",
    )
    .bind(server_id)
    .fetch_optional(db)
    .await
}

/// How run `run_id` ended, `None` while it runs or when there is no such run.
pub async fn run_outcome(
    db: &SqlitePool,
    run_id: &str,
) -> sqlx::Result<Option<Outcome>> {
    Ok(sqlx::query_scalar(
        "SELECT outcome FROM runs WHERE id = ? AND ended_at IS NOT NULL",
    )
    .bind(run_id)
    .fetch_optional(db)
    .await?
    .flatten())
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
        let port = create(&db, "a", |_| true).await.unwrap().unwrap();
        assert_eq!(port, i64::from(*SERVER_PORTS.start()));
        start_run(&db, "a", "run", 1).await.unwrap();
        assert_eq!(open_runs(&db, "a", i64::MAX).await.unwrap(), ["run"]);
        assert_eq!(servers_with_open_runs(&db).await.unwrap(), ["a"]);

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
        assert!(open_runs(&db, "a", i64::MAX).await.unwrap().is_empty());
        assert_eq!(
            last_outcome(&db, "a").await.unwrap(),
            Some(Outcome::Crashed)
        );
    }

    #[tokio::test]
    async fn new_servers_take_the_next_free_port() {
        let db = db::memory().await;
        let first = create(&db, "a", |_| true).await.unwrap().unwrap();
        let second = create(&db, "b", |_| true).await.unwrap().unwrap();
        assert_eq!(second, first + 1);
    }

    #[tokio::test]
    async fn ports_another_program_holds_are_skipped() {
        let db = db::memory().await;
        let start = *SERVER_PORTS.start();
        let taken = [start, start + 1];
        let port = create(&db, "a", |port| !taken.contains(&port))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(port, i64::from(start + 2));
        assert!(create(&db, "b", |_| false).await.unwrap().is_none());
    }

    #[test]
    fn a_port_in_use_is_not_free() {
        // Loopback only: tests must not open sockets Windows Firewall asks about.
        let local = Some(IpAddr::V4(Ipv4Addr::LOCALHOST));
        // A TCP listener, as another program's server would hold.
        let tcp = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(!port_free(tcp.local_addr().unwrap().port(), local));
        // UDP alone counts too: query and voice mods use it.
        let udp = std::net::UdpSocket::bind("127.0.0.1:0").unwrap();
        assert!(!port_free(udp.local_addr().unwrap().port(), local));
    }

    #[test]
    fn a_local_only_core_probes_only_its_own_address() {
        let local = IpAddr::V4(Ipv4Addr::LOCALHOST);
        assert_eq!(probed_ips(Some(local)), [local]);
        assert_eq!(
            probed_ips(None),
            [
                IpAddr::V4(Ipv4Addr::UNSPECIFIED),
                IpAddr::V6(Ipv6Addr::UNSPECIFIED)
            ]
        );
    }
}
