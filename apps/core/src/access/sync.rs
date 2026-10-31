//! Backend sync. The backend is a phone book and a notifier: Core tells it where this Core is and
//! who may reach it, on start and on change, never on a timer. Storage reports (see `storage`) are
//! the exception: they also go hourly, so a replica learns of new versions within the hour.
//!
//! A call that must reach the backend is owed: [`owe`] stores it in `backend_owed` in the same
//! transaction as the change, and [`run`] sends it, retrying with backoff until the backend
//! answers. On every start and reconnect Core also replaces the backend's whole directory and
//! reports its state, so a lost call cannot leave the two apart. When the backend says this Core
//! was removed from its account, Core un-pairs and shows a new pairing code.

use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
    time::Duration,
};

use reqwest::{Method, StatusCode};
use serde::Serialize;
use sqlx::{SqliteConnection, SqlitePool};
use tokio::sync::watch;
use tracing::{info, warn};

use super::{Access, Changed, now, pairing};
use crate::{
    api::backend::{AccessDirectory, OwedCall, StateReport, StorageAnswer},
    network::public_ip,
    storage::Storage,
};

/// How often Core looks for a new address.
const CHECK_EVERY: Duration = Duration::from_secs(5 * 60);
const CALL_TIMEOUT: Duration = Duration::from_secs(20);

/// Everything sync needs to know about this Core.
pub struct Sync {
    pub db: SqlitePool,
    pub http: reqwest::Client,
    pub backend_url: String,
    pub core_id: String,
    pub cert_fingerprint: String,
    /// The API port and the address it listens on.
    pub port: u16,
    pub bind: IpAddr,
    pub access: Arc<Access>,
    pub storage: Arc<Storage>,
}

/// Owe the backend `call`, in the transaction of the change that needs it. Wake sync with
/// `Access::owed` once it commits. A report already owed is not owed twice: it sends the state
/// as it is when it goes.
pub async fn owe(
    conn: &mut SqliteConnection,
    call: &OwedCall,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO backend_owed (call, created_at, next_attempt_at) SELECT ?1, ?2, ?2 \
         WHERE NOT (?3 AND EXISTS (SELECT 1 FROM backend_owed WHERE call = ?1))",
    )
    .bind(serde_json::to_string(call).expect("calls serialize"))
    .bind(now())
    .bind(matches!(call, OwedCall::ReportState | OwedCall::ReportStorage))
    .execute(conn)
    .await?;
    Ok(())
}

/// Forget the owner and everything that came from them, in the caller's transaction: their key,
/// owed calls, every session, and every role they gave. Send [`Changed::Everyone`] once it
/// commits. Used when the backend says this Core was removed, and by `DELETE /pairing`.
pub async fn unpair(conn: &mut SqliteConnection) -> sqlx::Result<()> {
    for statement in [
        "UPDATE core SET owner_user_id = NULL, backend_key = NULL WHERE id = 1",
        "DELETE FROM backend_owed",
        "DELETE FROM sessions",
        "DELETE FROM access",
    ] {
        sqlx::query(statement).execute(&mut *conn).await?;
    }
    Ok(())
}

/// Where the App reaches this Core.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Addresses {
    local: String,
    public: Option<String>,
}

/// Why sync lost the backend.
#[derive(Debug)]
enum Lost {
    /// No answer, or a failure worth retrying. Sync reconnects with backoff.
    Unreachable(String),
    /// The backend no longer knows this Core.
    Removed,
}

/// How one call went wrong.
#[derive(Debug)]
enum Failure {
    Lost(Lost),
    /// The backend refused this call for good. It is dropped.
    Refused(String),
}

/// Wait before the next try after `failures` failures in a row: 10 seconds, doubling, at most an
/// hour.
fn backoff(failures: u32) -> Duration {
    Duration::from_secs((10u64 << failures.min(9)).min(3600))
}

/// Keep the backend current for as long as Core runs: pair, connect, then send owed calls as they
/// come.
pub async fn run(sync: Arc<Sync>) {
    let (sender, addresses) = watch::channel(sync.addresses(None).await);
    tokio::spawn(Arc::clone(&sync).watch_addresses(sender));
    let mut failures = 0;
    loop {
        let address = addresses.borrow().local.clone();
        pairing::pair(&pairing::Pairing {
            db: &sync.db,
            http: &sync.http,
            backend_url: &sync.backend_url,
            core_id: &sync.core_id,
            address: &address,
            cert_fingerprint: &sync.cert_fingerprint,
            new_code: &sync.access.new_code,
        })
        .await;
        let Some(key) = sync.key().await else {
            // Paired but the key cannot be read: wait rather than spin.
            tokio::time::sleep(backoff(1)).await;
            continue;
        };
        let lost = match sync.connect(&key, &addresses).await {
            Ok(()) => {
                failures = 0;
                info!("connected to the backend");
                sync.serve_owed(&key, &addresses).await
            }
            Err(lost) => lost,
        };
        match lost {
            Lost::Removed => sync.unpair().await,
            Lost::Unreachable(problem) => {
                failures += 1;
                let wait = backoff(failures);
                warn!(%problem, "lost the backend, reconnecting in {}s", wait.as_secs());
                tokio::time::sleep(wait).await;
            }
        }
    }
}

impl Sync {
    async fn key(&self) -> Option<String> {
        sqlx::query_scalar("SELECT backend_key FROM core WHERE id = 1")
            .fetch_optional(&self.db)
            .await
            .ok()
            .flatten()
            .flatten()
    }

    /// This Core's addresses now. A failed public lookup keeps `previous`: no answer is not a
    /// change.
    async fn addresses(&self, previous: Option<&str>) -> Addresses {
        let local = if self.bind.is_unspecified() {
            public_ip::local_ipv4()
                .map_or(IpAddr::V4(Ipv4Addr::LOCALHOST), IpAddr::V4)
        } else {
            self.bind
        };
        let public = if local.is_loopback() {
            None
        } else {
            public_ip::find(match local {
                IpAddr::V4(v4) => Some(v4),
                IpAddr::V6(_) => None,
            })
            .await
            .map(|ip| self.url(ip))
            .or_else(|| previous.map(str::to_owned))
        };
        Addresses {
            local: self.url(local),
            public,
        }
    }

    fn url(&self, ip: IpAddr) -> String {
        format!("https://{}", SocketAddr::new(ip, self.port))
    }

    /// Look for a new address every few minutes and owe a state report when it changed.
    async fn watch_addresses(
        self: Arc<Self>,
        sender: watch::Sender<Addresses>,
    ) {
        loop {
            tokio::time::sleep(CHECK_EVERY).await;
            let previous = sender.borrow().public.clone();
            let next = self.addresses(previous.as_deref()).await;
            if *sender.borrow() == next {
                continue;
            }
            info!(local = %next.local, public = ?next.public, "this Core's address changed");
            sender.send_replace(next);
            let owed = async {
                let mut conn = self.db.acquire().await?;
                owe(&mut conn, &OwedCall::ReportState).await
            };
            match owed.await {
                Ok(()) => self.access.owed.notify_one(),
                Err(error) => warn!(%error, "could not owe a state report"),
            }
        }
    }

    /// Replace the backend's directory and report this Core's state, as on every start and
    /// reconnect.
    async fn connect(
        &self,
        key: &str,
        addresses: &watch::Receiver<Addresses>,
    ) -> Result<(), Lost> {
        let user_ids: Vec<String> = sqlx::query_scalar(
            "SELECT DISTINCT user_id FROM access ORDER BY user_id",
        )
        .fetch_all(&self.db)
        .await
        .map_err(|error| Lost::Unreachable(error.to_string()))?;
        let result = async {
            self.call(
                key,
                Method::PUT,
                "/core/access",
                &AccessDirectory { user_ids },
            )
            .await?;
            // This report covers any already owed.
            sqlx::query("DELETE FROM backend_owed WHERE call = ?")
                .bind(
                    serde_json::to_string(&OwedCall::ReportState)
                        .expect("calls serialize"),
                )
                .execute(&self.db)
                .await
                .map_err(|error| {
                    Failure::Lost(Lost::Unreachable(error.to_string()))
                })?;
            let state = self.state(&addresses.borrow());
            self.call(key, Method::POST, "/core/state", &state).await
        };
        match result.await {
            Ok(()) => Ok(()),
            Err(Failure::Lost(lost)) => Err(lost),
            // The backend refusing this Core's own state is not something a retry soon fixes.
            Err(Failure::Refused(problem)) => Err(Lost::Unreachable(problem)),
        }
    }

    fn state(&self, addresses: &Addresses) -> StateReport {
        StateReport {
            version: env!("CARGO_PKG_VERSION").into(),
            address: addresses.local.clone(),
            public_address: addresses.public.clone(),
            cert_fingerprint: self.cert_fingerprint.clone(),
            servers: Vec::new(),
        }
    }

    /// Send owed calls as they come due, until the backend is lost.
    async fn serve_owed(
        &self,
        key: &str,
        addresses: &watch::Receiver<Addresses>,
    ) -> Lost {
        loop {
            // The owner removed this Core from its account through the API: pair again.
            if matches!(pairing::paired(&self.db).await, Ok(false)) {
                return Lost::Removed;
            }
            let wait = match self.send_due(key, addresses).await {
                Ok(wait) => wait,
                Err(lost) => return lost,
            };
            tokio::select! {
                () = self.access.owed.notified() => {}
                () = tokio::time::sleep(wait) => {}
            }
        }
    }

    /// Send every owed call that is due, oldest first. Returns how long until the next one is.
    async fn send_due(
        &self,
        key: &str,
        addresses: &watch::Receiver<Addresses>,
    ) -> Result<Duration, Lost> {
        let db_error =
            |error: sqlx::Error| Lost::Unreachable(error.to_string());
        loop {
            let now = now();
            // Strictly in order: a later call never overtakes one waiting to retry, so an old
            // grant cannot undo a newer removal.
            let next: Option<(i64, String, i64, i64)> = sqlx::query_as(
                "SELECT id, call, attempts, next_attempt_at FROM backend_owed \
                 ORDER BY id LIMIT 1",
            )
            .fetch_optional(&self.db)
            .await
            .map_err(db_error)?;
            let Some((id, call, attempts, due)) = next else {
                return Ok(Duration::from_secs(86_400));
            };
            if due > now {
                return Ok(Duration::from_secs((due - now) as u64));
            }
            let current = addresses.borrow().clone();
            let sent = match serde_json::from_str(&call) {
                Ok(call) => self.send(key, &call, &current).await,
                Err(error) => {
                    Err(Failure::Refused(format!("unreadable call ({error})")))
                }
            };
            match sent {
                Ok(()) | Err(Failure::Refused(_)) => {
                    if let Err(Failure::Refused(problem)) = &sent {
                        warn!(%problem, call, "the backend refused an owed call; dropped");
                    }
                    sqlx::query("DELETE FROM backend_owed WHERE id = ?")
                        .bind(id)
                        .execute(&self.db)
                        .await
                        .map_err(db_error)?;
                }
                Err(Failure::Lost(lost)) => {
                    let attempts =
                        u32::try_from(attempts + 1).unwrap_or(u32::MAX);
                    sqlx::query(
                        "UPDATE backend_owed SET attempts = ?, next_attempt_at = ? WHERE id = ?",
                    )
                    .bind(attempts)
                    .bind(now + backoff(attempts).as_secs() as i64)
                    .bind(id)
                    .execute(&self.db)
                    .await
                    .map_err(db_error)?;
                    return Err(lost);
                }
            }
        }
    }

    async fn send(
        &self,
        key: &str,
        call: &OwedCall,
        addresses: &Addresses,
    ) -> Result<(), Failure> {
        match call {
            OwedCall::Grant(grant) => {
                // Removed again since: the removal that follows says all there is to say.
                let current: bool = sqlx::query_scalar(
                    "SELECT EXISTS (SELECT 1 FROM access WHERE server_id = ? AND user_id = ?)",
                )
                .bind(&grant.server_id)
                .bind(&grant.user)
                .fetch_one(&self.db)
                .await
                .map_err(|error| {
                    Failure::Lost(Lost::Unreachable(error.to_string()))
                })?;
                if !current {
                    return Ok(());
                }
                self.call(key, Method::POST, "/core/access", grant).await
            }
            OwedCall::Remove(remove) => {
                self.call(key, Method::DELETE, "/core/access", remove).await
            }
            OwedCall::ReportState => {
                self.call(
                    key,
                    Method::POST,
                    "/core/state",
                    &self.state(addresses),
                )
                .await
            }
            OwedCall::ReportStorage => {
                // A store that does not answer owes a report again once it runs.
                let Some(report) = self.storage.report().await else {
                    return Ok(());
                };
                let answer: StorageAnswer = self
                    .request(key, Method::POST, "/core/storage", &report)
                    .await?
                    .json()
                    .await
                    .map_err(|error| Failure::Refused(error.to_string()))?;
                self.storage.answer(answer);
                Ok(())
            }
        }
    }

    async fn call(
        &self,
        key: &str,
        method: Method,
        path: &str,
        body: &impl Serialize,
    ) -> Result<(), Failure> {
        self.request(key, method, path, body).await.map(drop)
    }

    /// Send one call and return the backend's answer when it succeeded.
    async fn request(
        &self,
        key: &str,
        method: Method,
        path: &str,
        body: &impl Serialize,
    ) -> Result<reqwest::Response, Failure> {
        let response = self
            .http
            .request(method, format!("{}{path}", self.backend_url))
            .bearer_auth(key)
            .timeout(CALL_TIMEOUT)
            .json(body)
            .send()
            .await
            .map_err(|error| {
                Failure::Lost(Lost::Unreachable(error.to_string()))
            })?;
        let status = response.status();
        if status.is_success() {
            return Ok(response);
        }
        if status == StatusCode::GONE {
            return Err(Failure::Lost(Lost::Removed));
        }
        let problem = format!(
            "{path} answered {status}: {}",
            response.text().await.unwrap_or_default()
        );
        // A refused key is retried: only the 410 above un-pairs, so a single 401 wipes nothing.
        if status.is_server_error()
            || matches!(
                status,
                StatusCode::UNAUTHORIZED
                    | StatusCode::REQUEST_TIMEOUT
                    | StatusCode::TOO_MANY_REQUESTS
            )
        {
            Err(Failure::Lost(Lost::Unreachable(problem)))
        } else {
            Err(Failure::Refused(problem))
        }
    }

    /// Forget the owner and everything that came from them: their key, owed calls, every session,
    /// and every role they gave. Servers and files stay. A new owner pairs the Core again.
    async fn unpair(&self) {
        let cleared = async {
            let mut tx = self.db.begin().await?;
            unpair(&mut tx).await?;
            tx.commit().await
        };
        match cleared.await {
            Ok(()) => {
                let _ = self.access.changed.send(Changed::Everyone);
                println!(
                    "\nThis Core was removed from its account. Its servers and files are untouched; pair it again to use it from the App.\n"
                );
            }
            Err(error) => {
                warn!(%error, "could not un-pair");
                tokio::time::sleep(backoff(1)).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use axum::{
        Json, Router,
        extract::State,
        http::{HeaderMap, Method as AxumMethod, StatusCode as AxumStatus},
        routing::any,
    };

    use super::*;
    use crate::{
        api::{
            access::Role,
            backend::{Grant, Remove},
        },
        db,
    };

    /// A backend that records every call and answers with the status at the front of `answers`,
    /// or 204 once it is empty.
    #[derive(Default)]
    struct Backend {
        calls: Mutex<Vec<(String, String, serde_json::Value)>>,
        answers: Mutex<Vec<u16>>,
    }

    async fn backend(answers: Vec<u16>) -> (Arc<Backend>, String) {
        let backend = Arc::new(Backend {
            answers: Mutex::new(answers),
            ..Backend::default()
        });
        let app = Router::new()
            .fallback(any(
                |State(backend): State<Arc<Backend>>,
                 method: AxumMethod,
                 uri: axum::http::Uri,
                 headers: HeaderMap,
                 Json(body): Json<serde_json::Value>| async move {
                    assert_eq!(headers["authorization"], "Bearer amc_key");
                    backend.calls.lock().unwrap().push((
                        method.to_string(),
                        uri.path().into(),
                        body,
                    ));
                    let mut answers = backend.answers.lock().unwrap();
                    let status = if answers.is_empty() {
                        204
                    } else {
                        answers.remove(0)
                    };
                    AxumStatus::from_u16(status).unwrap()
                },
            ))
            .with_state(Arc::clone(&backend));
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await });
        (backend, url)
    }

    async fn sync(url: String) -> Sync {
        let db = db::memory().await;
        let core_id = db::core_id(&db).await.unwrap();
        sqlx::query(
            "UPDATE core SET owner_user_id = 'owner', backend_key = 'amc_key'",
        )
        .execute(&db)
        .await
        .unwrap();
        let keys = crate::access::keys::load(
            db.clone(),
            crate::network::http().build().unwrap(),
            None,
        )
        .await
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        Sync {
            storage: Arc::new(Storage::new(
                db.clone(),
                crate::daemon::paths::Paths::new(dir.path()),
                core_id.clone(),
                None,
                false,
                Arc::new(tokio::sync::Notify::new()),
            )),
            db,
            http: crate::network::http().build().unwrap(),
            backend_url: url,
            core_id,
            cert_fingerprint: "f".repeat(64),
            port: 16662,
            bind: IpAddr::V4(Ipv4Addr::LOCALHOST),
            access: Arc::new(Access::new(keys)),
        }
    }

    async fn owed(sync: &Sync) -> Vec<(String, i64)> {
        sqlx::query_as("SELECT call, attempts FROM backend_owed ORDER BY id")
            .fetch_all(&sync.db)
            .await
            .unwrap()
    }

    fn addresses() -> watch::Receiver<Addresses> {
        watch::channel(Addresses {
            local: "https://127.0.0.1:16662".into(),
            public: None,
        })
        .1
    }

    #[tokio::test]
    async fn connecting_replaces_the_directory_and_reports_state() {
        let (backend, url) = backend(Vec::new()).await;
        let sync = sync(url).await;
        sqlx::query(
            "INSERT INTO servers (id, name, created_at, port, memory_mb) VALUES ('s1', 'A', 0, 25565, 1024), ('s2', 'B', 0, 25566, 1024); \
             INSERT INTO access (server_id, user_id, role, added_at) VALUES ('s1', 'b', 'viewer', 0), ('s2', 'b', 'editor', 0), ('s1', 'a', 'editor', 0)",
        )
        .execute(&sync.db)
        .await
        .unwrap();
        let mut conn = sync.db.acquire().await.unwrap();
        owe(&mut conn, &OwedCall::ReportState).await.unwrap();
        owe(&mut conn, &OwedCall::ReportState).await.unwrap();
        drop(conn);
        assert_eq!(
            owed(&sync).await.len(),
            1,
            "one state report is owed at most"
        );

        sync.connect("amc_key", &addresses()).await.unwrap();
        let calls = backend.calls.lock().unwrap().clone();
        assert_eq!(calls[0].0, "PUT");
        assert_eq!(calls[0].1, "/core/access");
        assert_eq!(calls[0].2, serde_json::json!({ "user_ids": ["a", "b"] }));
        assert_eq!(calls[1].1, "/core/state");
        assert_eq!(calls[1].2["address"], "https://127.0.0.1:16662");
        assert_eq!(calls[1].2["cert_fingerprint"], "f".repeat(64));
        assert!(
            owed(&sync).await.is_empty(),
            "the report covered the owed one"
        );
    }

    #[tokio::test]
    async fn owed_calls_retry_with_backoff_until_the_backend_answers() {
        let (backend, url) = backend(vec![503]).await;
        let sync = sync(url).await;
        sqlx::query(
            "INSERT INTO servers (id, name, created_at, port, memory_mb) VALUES ('s1', 'A', 0, 25565, 1024); \
             INSERT INTO access (server_id, user_id, role, added_at) VALUES ('s1', 'friend', 'editor', 0)",
        )
        .execute(&sync.db)
        .await
        .unwrap();
        let grant = OwedCall::Grant(Grant {
            user: "friend".into(),
            server_id: "s1".into(),
            server_name: "A".into(),
            role: Role::Editor,
        });
        let remove = OwedCall::Remove(Remove {
            user_id: "other".into(),
            server_id: "s1".into(),
            keep_core: false,
        });
        let mut conn = sync.db.acquire().await.unwrap();
        owe(&mut conn, &grant).await.unwrap();
        owe(&mut conn, &remove).await.unwrap();
        drop(conn);

        // The backend fails the first call: it waits, and nothing after it overtakes it.
        let lost = sync.send_due("amc_key", &addresses()).await.unwrap_err();
        assert!(matches!(lost, Lost::Unreachable(_)));
        assert_eq!(owed(&sync).await[0].1, 1);
        let wait = sync.send_due("amc_key", &addresses()).await.unwrap();
        assert_eq!(
            backend.calls.lock().unwrap().len(),
            1,
            "held behind the retry"
        );
        assert!(wait > Duration::from_secs(5) && wait <= backoff(1));

        // Once due, both go in order, and the list is empty.
        sqlx::query("UPDATE backend_owed SET next_attempt_at = 0")
            .execute(&sync.db)
            .await
            .unwrap();
        sync.send_due("amc_key", &addresses()).await.unwrap();
        assert!(owed(&sync).await.is_empty());
        let calls = backend.calls.lock().unwrap().clone();
        assert_eq!(calls.len(), 3);
        assert_eq!(
            (calls[1].0.as_str(), calls[1].2["user"].as_str()),
            ("POST", Some("friend"))
        );
        assert_eq!(
            (calls[2].0.as_str(), calls[2].2["user_id"].as_str()),
            ("DELETE", Some("other"))
        );
        assert_eq!(calls[1].2["role"], "editor");

        // A grant for someone removed since is not sent.
        sqlx::query("DELETE FROM access")
            .execute(&sync.db)
            .await
            .unwrap();
        let mut conn = sync.db.acquire().await.unwrap();
        owe(&mut conn, &grant).await.unwrap();
        drop(conn);
        sync.send_due("amc_key", &addresses()).await.unwrap();
        assert!(owed(&sync).await.is_empty());
        assert_eq!(backend.calls.lock().unwrap().len(), 3);
    }

    #[tokio::test]
    async fn a_refused_call_is_dropped_and_a_removed_core_unpairs() {
        let (_backend, url) = backend(vec![404, 410]).await;
        let sync = sync(url).await;
        sqlx::query(
            "INSERT INTO servers (id, name, created_at, port, memory_mb) VALUES ('s1', 'A', 0, 25565, 1024); \
             INSERT INTO access (server_id, user_id, role, added_at) VALUES ('s1', 'friend', 'editor', 0); \
             INSERT INTO sessions VALUES ('x', x'00', 'owner', 'Laptop', 0, 0, 9999999999)",
        )
        .execute(&sync.db)
        .await
        .unwrap();
        let mut conn = sync.db.acquire().await.unwrap();
        owe(&mut conn, &OwedCall::ReportState).await.unwrap();
        drop(conn);
        sync.send_due("amc_key", &addresses()).await.unwrap();
        assert!(owed(&sync).await.is_empty(), "a 404 is never retried");

        let mut changed = sync.access.changed.subscribe();
        let lost = sync.connect("amc_key", &addresses()).await.unwrap_err();
        assert!(matches!(lost, Lost::Removed));
        sync.unpair().await;
        assert_eq!(changed.recv().await.unwrap(), Changed::Everyone);
        assert_eq!(sync.key().await, None);
        for table in ["access", "sessions"] {
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(format!(
                "SELECT COUNT(*) FROM {table}"
            )))
            .fetch_one(&sync.db)
            .await
            .unwrap();
            assert_eq!(count, 0, "{table}");
        }
        let servers: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM servers")
            .fetch_one(&sync.db)
            .await
            .unwrap();
        assert_eq!(servers, 1, "servers stay");
    }

    #[test]
    fn backoff_doubles_up_to_an_hour() {
        assert_eq!(backoff(0), Duration::from_secs(10));
        assert_eq!(backoff(1), Duration::from_secs(20));
        assert_eq!(backoff(30), Duration::from_secs(3600));
    }
}
