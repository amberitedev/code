//! Core's side of the store: start `copal store` while storage is on and restart it when it
//! dies, forward `/store/...` to it, keep it to the owner's settings, and report what it holds to
//! the backend (`POST /core/storage`, sent by `access::sync`).
//!
//! In development the store runs as the account running Core. Running it as an OS account of its
//! own, with a folder Core cannot read, is the install-time follow-up the design asks for.

use std::{
    collections::HashSet,
    net::IpAddr,
    process::Stdio,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use axum::{
    Json,
    body::Body,
    extract::Request,
    http::{HeaderValue, Method, StatusCode, Uri, header},
    response::{IntoResponse, Response},
};
use hyper_util::{
    client::legacy::{Client, connect::HttpConnector},
    rt::TokioExecutor,
};
use sqlx::SqlitePool;
use tokio::{
    io::{AsyncBufReadExt, BufReader},
    process::{Child, Command},
    sync::{Notify, watch},
};
use tracing::{info, warn};

use super::{
    ADMIN_TOKEN_ENV, ALLOW_PRIVATE_ENV, AdminStatus, Caps, Drop, objects::Hash,
};
use crate::{
    access::sync,
    api::{
        backend::{OwedCall, PullOrder, StorageAnswer, StorageReport},
        error::{ApiError, ErrorCode},
        storage::{StorageSettings, StorageStatus},
    },
    backups::space,
    daemon::paths::Paths,
};

const GIB: u64 = 1024 * 1024 * 1024;
/// How long the store has to say where it listens.
const START_TIMEOUT: Duration = Duration::from_secs(30);
/// Reports after uploads wait this long, so a burst of them makes one.
const REPORT_DEBOUNCE: Duration = Duration::from_secs(2);
const REPORT_EVERY: Duration = Duration::from_secs(24 * 3600);
/// Headers about one connection, which a proxy must not pass on.
const HOP_HEADERS: [&str; 8] = [
    "connection",
    "keep-alive",
    "proxy-connection",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
];

/// The cap when the owner set none: a tenth of the free space, at most 20 GiB and at least
/// 10 GiB.
fn default_cap(free: u64) -> u64 {
    (free / 10).clamp(10 * GIB, 20 * GIB)
}

/// The owner's storage settings, as the `core` table keeps them.
#[derive(Debug, Clone, Copy)]
struct Settings {
    enabled: bool,
    cap: u64,
    egress_cap: u64,
}

pub struct Storage {
    db: SqlitePool,
    paths: Paths,
    core_id: String,
    backend_url: Option<String>,
    /// Installed as a service, which the backend requires of a Core holding others' files.
    service: bool,
    default_cap: u64,
    /// What the store's `/admin/` routes take. New on every Core start.
    token: String,
    /// The store's port while it runs.
    port: watch::Sender<Option<u16>>,
    /// The settings changed.
    changed: Notify,
    /// A forwarded upload may have stored something.
    stored: Notify,
    /// Wakes backend sync (`Access::owed`).
    owed: Arc<Notify>,
    pulling: Mutex<HashSet<String>>,
    forwarder: Client<HttpConnector, Body>,
    http: reqwest::Client,
}

impl Storage {
    pub fn new(
        db: SqlitePool,
        paths: Paths,
        core_id: String,
        backend_url: Option<String>,
        service: bool,
        owed: Arc<Notify>,
    ) -> Self {
        let free = space::free(&paths.store())
            .or_else(|_| space::free(paths.root()))
            .unwrap_or(0);
        let mut token = [0u8; 32];
        ring::rand::SecureRandom::fill(
            &ring::rand::SystemRandom::new(),
            &mut token,
        )
        .expect("the OS has randomness");
        Self {
            db,
            paths,
            core_id,
            backend_url,
            service,
            default_cap: default_cap(free),
            token: hex::encode(token),
            port: watch::channel(None).0,
            changed: Notify::new(),
            stored: Notify::new(),
            owed,
            pulling: Mutex::default(),
            forwarder: Client::builder(TokioExecutor::new()).build_http(),
            http: reqwest::Client::builder()
                .no_proxy()
                .build()
                .expect("a plain HTTP client builds"),
        }
    }

    /// Run the store while storage is on, and report to the backend after uploads and daily.
    pub fn start(self: &Arc<Self>) {
        tokio::spawn(Arc::clone(self).supervise());
        let storage = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                storage.stored.notified().await;
                tokio::time::sleep(REPORT_DEBOUNCE).await;
                storage.owe_report().await;
            }
        });
        let storage = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                tokio::time::sleep(REPORT_EVERY).await;
                storage.owe_report().await;
            }
        });
    }

    async fn settings(&self) -> sqlx::Result<Settings> {
        let (enabled, cap, egress_cap): (bool, Option<i64>, i64) = sqlx::query_as(
            "SELECT storage_enabled, storage_cap, storage_egress_cap FROM core WHERE id = 1",
        )
        .fetch_one(&self.db)
        .await?;
        Ok(Settings {
            enabled,
            cap: cap.map_or(self.default_cap, |cap| cap.max(0) as u64),
            egress_cap: egress_cap.max(0) as u64,
        })
    }

    fn port(&self) -> Option<u16> {
        *self.port.borrow()
    }

    /// Keep the store running while storage is on: start it, start it again with backoff when it
    /// exits, and stop it when storage is turned off.
    async fn supervise(self: Arc<Self>) {
        let mut failures = 0u32;
        loop {
            let settings = match self.settings().await {
                Ok(settings) => settings,
                Err(error) => {
                    warn!(%error, "could not read the storage settings");
                    tokio::time::sleep(Duration::from_secs(10)).await;
                    continue;
                }
            };
            if !settings.enabled {
                self.port.send_replace(None);
                self.changed.notified().await;
                continue;
            }
            let started = Instant::now();
            let (mut child, port) = match self.spawn(settings).await {
                Ok(running) => running,
                Err(error) => {
                    failures += 1;
                    warn!(%error, "could not start the store");
                    tokio::time::sleep(backoff(failures)).await;
                    continue;
                }
            };
            let stdin = child.stdin.take();
            self.port.send_replace(Some(port));
            info!(port, "the store for shared files is running");
            if self.admin_status().await.is_some() {
                self.owe_report().await;
            }
            let exited = tokio::select! {
                status = child.wait() => {
                    warn!(?status, "the store stopped");
                    true
                }
                () = self.turned_off() => false,
            };
            self.port.send_replace(None);
            if !exited {
                // Closing stdin is how the store is asked to stop.
                drop(stdin);
                if tokio::time::timeout(Duration::from_secs(10), child.wait())
                    .await
                    .is_err()
                {
                    let _ = child.kill().await;
                }
                info!("storage is off; the store stopped");
                continue;
            }
            failures = if started.elapsed() > Duration::from_secs(60) {
                1
            } else {
                failures + 1
            };
            tokio::time::sleep(backoff(failures)).await;
        }
    }

    /// Resolves once the owner turns storage off.
    async fn turned_off(&self) {
        loop {
            self.changed.notified().await;
            if self
                .settings()
                .await
                .is_ok_and(|settings| !settings.enabled)
            {
                return;
            }
        }
    }

    /// Start `copal store` and read the port it says it listens on.
    async fn spawn(&self, settings: Settings) -> std::io::Result<(Child, u16)> {
        let mut command = Command::new(std::env::current_exe()?);
        command
            .arg("store")
            .arg("--data-dir")
            .arg(self.paths.store())
            .args(["--core-id", &self.core_id])
            .args([
                "--backend-url",
                self.backend_url.as_deref().unwrap_or("none"),
            ])
            .args(["--cap", &settings.cap.to_string()])
            .args(["--egress-cap", &settings.egress_cap.to_string()])
            .env(ADMIN_TOKEN_ENV, &self.token)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        // Core may have no console; the store must not open one.
        #[cfg(windows)]
        command.creation_flags(
            windows_sys::Win32::System::Threading::CREATE_NO_WINDOW,
        );
        // A local backend means local Cores, whose addresses are private.
        if self
            .backend_url
            .as_deref()
            .is_some_and(|url| url.starts_with("http://"))
        {
            command.env(ALLOW_PRIVATE_ENV, "1");
        }
        let mut child = command.spawn()?;
        // The store's log goes into Core's.
        let stderr = child.stderr.take().expect("stderr is piped");
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = lines.next_line().await {
                info!(target: "copal::store", "{line}");
            }
        });
        let stdout = child.stdout.take().expect("stdout is piped");
        let line = tokio::time::timeout(
            START_TIMEOUT,
            BufReader::new(stdout).lines().next_line(),
        )
        .await;
        let port = match line {
            Ok(Ok(Some(line))) => line
                .strip_prefix("listening ")
                .and_then(|port| port.trim().parse().ok()),
            _ => None,
        };
        match port {
            Some(port) => Ok((child, port)),
            None => {
                let _ = child.kill().await;
                Err(std::io::Error::other(
                    "the store did not say where it listens",
                ))
            }
        }
    }

    fn admin_url(&self, path: &str) -> Option<String> {
        self.port()
            .map(|port| format!("http://127.0.0.1:{port}{path}"))
    }

    /// The store's status. A store that is starting, as right after storage is turned on, gets
    /// a few seconds to come up first, so its holdings are not reported as nothing.
    async fn admin_status(&self) -> Option<AdminStatus> {
        let mut port = self.port.subscribe();
        let _ = tokio::time::timeout(
            Duration::from_secs(10),
            port.wait_for(Option::is_some),
        )
        .await;
        let url = self.admin_url("/admin/status")?;
        let answer = self
            .http
            .get(url)
            .bearer_auth(&self.token)
            .timeout(Duration::from_secs(10))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        match answer {
            Ok(answer) => answer.json().await.ok(),
            Err(error) => {
                warn!(%error, "the store did not answer its status");
                None
            }
        }
    }

    async fn admin_post(
        &self,
        path: &str,
        body: &impl serde::Serialize,
    ) -> Result<(), String> {
        let url = self.admin_url(path).ok_or("the store is not running")?;
        self.http
            .post(url)
            .bearer_auth(&self.token)
            .timeout(Duration::from_secs(30))
            .json(body)
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map(drop)
            .map_err(|error| error.to_string())
    }

    /// `GET /storage`.
    pub async fn status(&self) -> Result<StorageStatus, ApiError> {
        let settings = self.settings().await?;
        let running = if settings.enabled {
            self.admin_status().await
        } else {
            None
        };
        let free = match &running {
            Some(status) => status.free,
            None => space::free(&self.paths.store()).unwrap_or(0),
        };
        Ok(StorageStatus {
            enabled: settings.enabled,
            cap: settings.cap,
            egress_cap: settings.egress_cap,
            used: running.as_ref().map_or(0, |status| status.used),
            free,
            egress_today: running
                .as_ref()
                .map_or(0, |status| status.egress_today),
            service: self.service,
            versions: running.as_ref().map_or(0, |status| {
                u32::try_from(status.versions.len()).unwrap_or(u32::MAX)
            }),
        })
    }

    /// `PATCH /storage`: keep the settings given, pass the caps to a running store, start or stop
    /// it, and tell the backend.
    pub async fn change(
        &self,
        change: StorageSettings,
    ) -> Result<StorageStatus, ApiError> {
        if change.cap.is_some_and(|cap| cap <= 0)
            || change.egress_cap.is_some_and(|cap| cap < 0)
        {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "The caps are bytes: more than zero for storage, zero or more for egress.",
            ));
        }
        sqlx::query(
            "UPDATE core SET storage_enabled = coalesce(?, storage_enabled), \
             storage_cap = coalesce(?, storage_cap), \
             storage_egress_cap = coalesce(?, storage_egress_cap) WHERE id = 1",
        )
        .bind(change.enabled)
        .bind(change.cap)
        .bind(change.egress_cap)
        .execute(&self.db)
        .await?;
        let settings = self.settings().await?;
        if self.port().is_some() {
            let caps = Caps {
                cap: settings.cap,
                egress_cap: settings.egress_cap,
            };
            if let Err(error) = self.admin_post("/admin/config", &caps).await {
                warn!(%error, "could not pass the new caps to the store");
            }
        }
        self.changed.notify_one();
        self.owe_report().await;
        self.status().await
    }

    /// Owe the backend a storage report. A Core without a backend reports nothing.
    pub async fn owe_report(&self) {
        if self.backend_url.is_none() {
            return;
        }
        let owed = async {
            let mut conn = self.db.acquire().await?;
            sync::owe(&mut conn, &OwedCall::ReportStorage).await
        };
        match owed.await {
            Ok(()) => self.owed.notify_one(),
            Err(error) => warn!(%error, "could not owe a storage report"),
        }
    }

    /// What `POST /core/storage` sends now. `None` while storage is on but the store does not
    /// answer: its next start owes a report again.
    pub async fn report(&self) -> Option<StorageReport> {
        let settings = self.settings().await.ok()?;
        if !settings.enabled {
            return Some(StorageReport {
                enabled: false,
                service: self.service,
                cap: settings.cap,
                used: 0,
                free: space::free(&self.paths.store()).unwrap_or(0),
                egress_cap: settings.egress_cap,
                egress_today: 0,
                versions: Vec::new(),
            });
        }
        let status = self.admin_status().await?;
        Some(StorageReport {
            enabled: true,
            service: self.service,
            cap: status.cap,
            used: status.used,
            free: status.free,
            egress_cap: status.egress_cap,
            egress_today: status.egress_today,
            versions: status.versions,
        })
    }

    /// Do what the backend answered a report with: drop versions and pull others, in the
    /// background. A finished pull owes a report of its own.
    pub fn answer(self: &Arc<Self>, answer: StorageAnswer) {
        if answer.drop.is_empty() && answer.pull.is_empty() {
            return;
        }
        let storage = Arc::clone(self);
        tokio::spawn(async move {
            if !answer.drop.is_empty() {
                let roots = Drop { roots: answer.drop };
                if let Err(error) =
                    storage.admin_post("/admin/drop", &roots).await
                {
                    warn!(%error, "could not drop versions");
                }
            }
            for order in answer.pull {
                let new = storage
                    .pulling
                    .lock()
                    .expect("not poisoned")
                    .insert(order.root.clone());
                if new {
                    tokio::spawn(Arc::clone(&storage).pull(order));
                }
            }
        });
    }

    async fn pull(self: Arc<Self>, order: PullOrder) {
        let complete = async {
            let root = Hash::parse(&order.root).ok_or("not a version")?;
            let url = self
                .admin_url(&format!("/store/versions/{root}/pull"))
                .ok_or("the store is not running")?;
            let answer: serde_json::Value = self
                .http
                .post(url)
                .bearer_auth(&order.ticket)
                .send()
                .await
                .map_err(|error| error.to_string())?
                .json()
                .await
                .map_err(|error| error.to_string())?;
            match answer["complete"].as_bool() {
                Some(complete) => Ok(complete),
                None => Err(answer.to_string()),
            }
        };
        let result: Result<bool, String> = complete.await;
        self.pulling
            .lock()
            .expect("not poisoned")
            .remove(&order.root);
        match result {
            Ok(true) => {
                info!(root = %order.root, "pulled a version");
                self.owe_report().await;
            }
            Ok(false) => warn!(root = %order.root, "a pull ended incomplete"),
            Err(error) => warn!(root = %order.root, %error, "a pull failed"),
        }
    }

    /// Forward a request under `/store/` to the store as it is, with `caller` as
    /// `X-Forwarded-For`, and stream the answer back. Core never reads either body.
    pub async fn forward(
        &self,
        mut request: Request,
        caller: IpAddr,
    ) -> Response {
        let Some(port) = self.port() else {
            return unavailable("Storage is off on this Core.");
        };
        let path = request
            .uri()
            .path_and_query()
            .map_or("/", |path| path.as_str())
            .to_owned();
        let uploaded = request.method() == Method::POST
            && !request.uri().path().ends_with("/have");
        *request.uri_mut() =
            Uri::try_from(format!("http://127.0.0.1:{port}{path}"))
                .expect("a path Core received is a valid URI");
        let headers = request.headers_mut();
        for name in HOP_HEADERS {
            headers.remove(name);
        }
        headers.remove(header::HOST);
        headers.insert(
            "x-forwarded-for",
            HeaderValue::from_str(&caller.to_string())
                .expect("an IP is a valid header"),
        );
        match self.forwarder.request(request).await {
            Ok(response) => {
                if uploaded && response.status().is_success() {
                    self.stored.notify_one();
                }
                let (mut parts, body) = response.into_parts();
                for name in HOP_HEADERS {
                    parts.headers.remove(name);
                }
                Response::from_parts(parts, Body::new(body))
            }
            Err(error) => {
                warn!(%error, "the store did not answer a forwarded request");
                unavailable(
                    "This Core's storage did not answer. Try again shortly.",
                )
            }
        }
    }
}

fn unavailable(description: &str) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        Json(ApiError::new(ErrorCode::Conflict, description)),
    )
        .into_response()
}

/// Wait before starting the store again after `failures` failed starts in a row: 2 seconds,
/// doubling, at most five minutes.
fn backoff(failures: u32) -> Duration {
    Duration::from_secs((1u64 << failures.min(9)).min(300))
}

#[cfg(test)]
pub mod tests {
    use axum::{Router, http::HeaderMap, routing::any};
    use http_body_util::BodyExt;

    use super::*;
    use crate::db;

    /// Storage that is never started, as tests use it.
    pub async fn storage(dir: &std::path::Path) -> Arc<Storage> {
        let db = db::memory().await;
        let core_id = db::core_id(&db).await.unwrap();
        Arc::new(Storage::new(
            db,
            Paths::new(dir),
            core_id,
            None,
            false,
            Arc::new(Notify::new()),
        ))
    }

    #[test]
    fn the_default_cap_is_a_tenth_of_free_space_within_bounds() {
        assert_eq!(default_cap(0), 10 * GIB);
        assert_eq!(default_cap(150 * GIB), 15 * GIB);
        assert_eq!(default_cap(1000 * GIB), 20 * GIB);
    }

    #[tokio::test]
    async fn requests_are_forwarded_with_the_caller_s_address() {
        let dir = tempfile::tempdir().unwrap();
        let storage = storage(dir.path()).await;
        let request = || {
            Request::builder()
                .method(Method::POST)
                .uri("/store/versions/x?y=1")
                .header("x-forwarded-for", "1.1.1.1")
                .body(Body::from("the body"))
                .unwrap()
        };
        let off = storage
            .forward(request(), IpAddr::from([203, 0, 113, 5]))
            .await;
        assert_eq!(off.status(), StatusCode::SERVICE_UNAVAILABLE);

        let echo = Router::new().fallback(any(
            |uri: Uri, headers: HeaderMap, body: String| async move {
                format!(
                    "{uri} {} {body}",
                    headers["x-forwarded-for"].to_str().unwrap()
                )
            },
        ));
        let listener =
            tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        storage
            .port
            .send_replace(Some(listener.local_addr().unwrap().port()));
        tokio::spawn(async move { axum::serve(listener, echo).await });
        let response = storage
            .forward(request(), IpAddr::from([203, 0, 113, 5]))
            .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(body, "/store/versions/x?y=1 203.0.113.5 the body");
    }
}
