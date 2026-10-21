//! What Core does while a server runs: read its console for players, probe it for freezes (see
//! `insight`), and start it again after a crash.

use std::{
    collections::HashMap,
    pin::Pin,
    sync::{
        Arc, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::{Duration, Instant},
};

use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{
    Lifecycle, Phase, ServerError, Servers, console,
    insight::{self, Insight, Probes},
    records::{self, Outcome},
    restarts,
};
use crate::access::now;

/// Crashes within [`CRASH_WINDOW`] after which Core leaves a server stopped.
pub const CRASH_LIMIT: i64 = 3;
pub const CRASH_WINDOW: Duration = Duration::from_secs(10 * 60);
/// How long Core waits before starting a crashed server again.
const CRASH_DELAY: Duration = Duration::from_secs(5);

/// Tells one run's console follower from the next run's.
static RUNS: AtomicU64 = AtomicU64::new(0);

/// What Core knows about a running server, for the run `run`.
pub(super) struct Watched {
    run: u64,
    pub insight: Insight,
}

/// Whether a server that crashed `recent` times within [`CRASH_WINDOW`], counting the crash just
/// now, may start again.
pub fn may_restart_after_crash(recent: i64) -> bool {
    recent < CRASH_LIMIT
}

impl Servers {
    /// A run started or was adopted: follow its console and tell subscribers. An adopted run
    /// printed its "Done" line before this Core read the console, so it counts as ready.
    pub(super) fn on_running(&self, id: &str, adopted: bool) {
        let run = RUNS.fetch_add(1, Ordering::Relaxed);
        self.watched().insert(
            id.into(),
            Watched {
                run,
                insight: Insight {
                    ready: adopted,
                    ..Insight::default()
                },
            },
        );
        let _ = self.events.send(Lifecycle::Running(id.into()));
        if let Some(me) = self.me.upgrade() {
            tokio::spawn(me.follow(id.to_owned(), run));
        }
    }

    /// A run ended: forget its console, tell subscribers, and start it again after a crash.
    pub(super) async fn ended(&self, id: &str) {
        self.watched().remove(id);
        let _ = self.events.send(Lifecycle::Ended(id.into()));
        if let Ok(Some(run)) = records::last_ended(&self.db, id).await {
            self.after_crash(id, &run).await;
        }
    }

    fn watched(&self) -> MutexGuard<'_, HashMap<String, Watched>> {
        self.insight.lock().expect("not poisoned")
    }

    pub(super) fn retries(&self) -> MutexGuard<'_, HashMap<String, String>> {
        self.retries.lock().expect("not poisoned")
    }

    /// What a running server's console said, `None` when it does not run.
    pub fn insight(&self, id: &str) -> Option<Insight> {
        self.watched()
            .get(id)
            .map(|watched| watched.insight.clone())
    }

    /// Which run of the server Core follows, `None` when it does not run. A task that outlives
    /// a run checks this before acting for it.
    pub fn run_of(&self, id: &str) -> Option<u64> {
        self.watched().get(id).map(|watched| watched.run)
    }

    /// Whether Core stopped starting a server again because it crashed too often.
    pub fn gave_up(&self, id: &str) -> bool {
        self.gave_up.lock().expect("not poisoned").contains(id)
    }

    /// The servers that are running, not starting, stopping, or unmanaged.
    pub fn running(&self) -> Vec<String> {
        self.live()
            .iter()
            .filter(|(_, live)| live.phase == Phase::Running)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Drop the server's pending crash restart. Every owner action does, so a server someone
    /// stopped, started, or is changing does not start again by itself. Whether there was one.
    pub(super) fn cancel_retry(&self, id: &str) -> bool {
        self.retries().remove(id).is_some()
    }

    /// After run `run` ended: if it crashed, start the server again in [`CRASH_DELAY`], unless
    /// crash restart is off for it or it crashed [`CRASH_LIMIT`] times in [`CRASH_WINDOW`]. Then
    /// it stays stopped, with its crash report in its runs.
    ///
    /// Boxed, with `Send` spelled out: a crash restart can crash in turn, and the compiler
    /// cannot see through that cycle of futures by itself.
    pub(super) fn after_crash<'a>(
        &'a self,
        id: &'a str,
        run: &'a str,
    ) -> Pin<Box<dyn Future<Output = ()> + Send + 'a>> {
        Box::pin(async move {
            if records::run_outcome(&self.db, run).await.ok().flatten()
                != Some(Outcome::Crashed)
                || !self.crash_restart_on(id).await
            {
                return;
            }
            let since = now() - CRASH_WINDOW.as_secs() as i64;
            let recent: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM runs WHERE server_id = ? AND outcome = 'crashed' AND ended_at >= ?",
            )
            .bind(id)
            .bind(since)
            .fetch_one(&self.db)
            .await
            .unwrap_or(CRASH_LIMIT);
            if !may_restart_after_crash(recent) {
                warn!(
                    server = %id,
                    "crashed {recent} times in 10 minutes; left stopped with its crash report"
                );
                self.gave_up.lock().expect("not poisoned").insert(id.into());
                return;
            }
            self.retries().insert(id.into(), run.into());
            let Some(me) = self.me.upgrade() else { return };
            let (id, run) = (id.to_owned(), run.to_owned());
            tokio::spawn(async move {
                tokio::time::sleep(CRASH_DELAY).await;
                if let Some(Err(error)) = me.retry(&id, &run).await {
                    warn!(server = %id, %error, "could not start again after a crash");
                }
            });
        })
    }

    /// The crash restart [`after_crash`](Self::after_crash) queued for run `run`, once its
    /// delay is over. `None` when it no longer stands: an owner action cancelled it, or crash
    /// restart was turned off meanwhile.
    pub(super) async fn retry(
        &self,
        id: &str,
        run: &str,
    ) -> Option<Result<(), ServerError>> {
        if self.retries().get(id).map(String::as_str) != Some(run) {
            return None;
        }
        if !self.crash_restart_on(id).await {
            self.retries().remove(id);
            return None;
        }
        info!(server = %id, "starting again after a crash");
        // `start` takes the retry under the lock it starts with, so a stop that came first
        // wins.
        Some(self.start_inner(id, Some(run)).await)
    }

    async fn crash_restart_on(&self, id: &str) -> bool {
        restarts::get(&self.db, id)
            .await
            .is_ok_and(|restarts| restarts.crash)
    }

    /// Follow run `run`'s console until it ends: players from its lines, a probe every minute,
    /// and `list` once it is ready and every few minutes after.
    async fn follow(self: Arc<Self>, id: String, run: u64) {
        let path = self.paths.console_log(&id);
        let end = console::tail(&path, 0).map_or(0, |(_, end)| end);
        let (sender, mut lines) = mpsc::channel(256);
        console::follow(path, end, sender);
        let mut probe = tokio::time::interval(insight::PROBE_EVERY);
        let mut list = tokio::time::interval(insight::LIST_EVERY);
        let mut probes = Probes::default();
        loop {
            let ask = tokio::select! {
                line = lines.recv() => {
                    let Some(line) = line else { return };
                    let at = Instant::now();
                    let mut watched = self.watched();
                    let Some(this) = watched.get_mut(&id).filter(|this| this.run == run) else {
                        return;
                    };
                    let was_ready = this.insight.ready;
                    this.insight.read(&line, at);
                    if probes.answered(&line, at) {
                        this.insight.answered_at = Some(now());
                        this.insight.frozen = false;
                    }
                    (!was_ready && this.insight.ready).then(|| {
                        this.insight.ask_list(at);
                        "list".to_owned()
                    })
                }
                _ = probe.tick() => {
                    let mut watched = self.watched();
                    let Some(this) = watched.get_mut(&id).filter(|this| this.run == run) else {
                        return;
                    };
                    let was_frozen = this.insight.frozen;
                    this.insight.check_frozen(now());
                    if this.insight.frozen && !was_frozen {
                        warn!(server = %id, "stopped answering; marked frozen");
                    }
                    this.insight.ready.then(|| probes.send(Instant::now()))
                }
                _ = list.tick() => {
                    let mut watched = self.watched();
                    let Some(this) = watched.get_mut(&id).filter(|this| this.run == run) else {
                        return;
                    };
                    this.insight.ready.then(|| {
                        this.insight.ask_list(Instant::now());
                        "list".to_owned()
                    })
                }
            };
            if let Some(command) = ask
                && self.phase(&id) == Some(Phase::Running)
            {
                let _ = self.command(&id, &command).await;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{daemon::paths::Paths, db, servers::restarts::Restarts};

    #[test]
    fn three_crashes_in_ten_minutes_is_the_limit() {
        assert!(may_restart_after_crash(1));
        assert!(may_restart_after_crash(2));
        assert!(!may_restart_after_crash(3));
        assert!(!may_restart_after_crash(4));
    }

    async fn crashed(
        db: &sqlx::SqlitePool,
        server: &str,
        ended_at: i64,
    ) -> String {
        let run = uuid::Uuid::new_v4().to_string();
        records::start_run(db, server, &run, ended_at - 1)
            .await
            .unwrap();
        sqlx::query(
            "UPDATE runs SET ended_at = ?, outcome = 'crashed' WHERE id = ?",
        )
        .bind(ended_at)
        .bind(&run)
        .execute(db)
        .await
        .unwrap();
        run
    }

    async fn servers() -> (tempfile::TempDir, Arc<Servers>) {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        let servers = Servers::open(paths, db::memory().await).await.unwrap();
        (dir, servers)
    }

    #[tokio::test]
    async fn a_server_that_keeps_crashing_is_left_stopped() {
        let (_dir, servers) = servers().await;
        let db = servers.db.clone();
        let server = servers.create("A").await.unwrap().id;
        // An old crash does not count.
        crashed(&db, &server, now() - 3600).await;
        crashed(&db, &server, now() - 120).await;
        let run = crashed(&db, &server, now() - 60).await;
        servers.after_crash(&server, &run).await;
        assert!(!servers.gave_up(&server), "two recent crashes start again");
        let run = crashed(&db, &server, now()).await;
        servers.after_crash(&server, &run).await;
        assert!(servers.gave_up(&server));

        // With crash restart off, Core never tries, so it never gives up either.
        let other = servers.create("B").await.unwrap().id;
        restarts::set(
            &db,
            &other,
            &Restarts {
                crash: false,
                ..Restarts::default()
            },
        )
        .await
        .unwrap();
        for ago in [30, 20, 10] {
            let run = crashed(&db, &other, now() - ago).await;
            servers.after_crash(&other, &run).await;
        }
        assert!(!servers.gave_up(&other));
        assert!(servers.retries().get(&other).is_none());
    }

    #[tokio::test]
    async fn a_crash_while_starting_queues_one_restart() {
        let (_dir, servers) = servers().await;
        let db = servers.db.clone();
        let server = servers.create("A").await.unwrap().id;
        // A start refused before it ran anything is not a crash, whatever the last run did.
        crashed(&db, &server, now() - 60).await;
        assert!(matches!(
            servers.start(&server).await,
            Err(ServerError::Eula)
        ));
        assert!(servers.retries().is_empty());

        // The shim died before Java ran: no status file, so the run crashed.
        records::start_run(&db, &server, "boot", now())
            .await
            .unwrap();
        servers.start_failed(&server, "boot").await;
        assert_eq!(
            records::run_outcome(&db, "boot").await.unwrap(),
            Some(Outcome::Crashed)
        );
        assert_eq!(
            servers.retries().get(&server).map(String::as_str),
            Some("boot")
        );
        // The watcher never saw the run, and the scan finds nothing open: one restart.
        servers.scan().await.unwrap();
        assert_eq!(servers.retries().len(), 1);
    }

    #[tokio::test]
    async fn a_queued_crash_restart_is_bound_to_its_run() {
        let (_dir, servers) = servers().await;
        let db = servers.db.clone();
        let server = servers.create("A").await.unwrap().id;
        let run = crashed(&db, &server, now()).await;
        servers.after_crash(&server, &run).await;
        // Pending until it runs, so an update waits for it.
        assert!(servers.unsettled());

        // A stop cancels it, and counts as done.
        servers.stop(&server).await.unwrap();
        assert!(!servers.unsettled());
        assert!(servers.retry(&server, &run).await.is_none());
        assert!(matches!(
            servers.stop(&server).await,
            Err(ServerError::NotRunning)
        ));

        // A retry for another run does nothing.
        servers.after_crash(&server, &run).await;
        assert!(servers.retry(&server, "another").await.is_none());

        // Crash restart turned off meanwhile.
        restarts::set(
            &db,
            &server,
            &Restarts {
                crash: false,
                ..Restarts::default()
            },
        )
        .await
        .unwrap();
        assert!(servers.retry(&server, &run).await.is_none());
        assert!(!servers.unsettled());

        // One that still stands starts the server. Here the EULA stops it, so it tried.
        restarts::set(&db, &server, &Restarts::default())
            .await
            .unwrap();
        servers.after_crash(&server, &run).await;
        assert!(matches!(
            servers.retry(&server, &run).await,
            Some(Err(ServerError::Eula))
        ));
        assert!(!servers.unsettled());
    }
}
