//! What Core does while a server runs: read its console for players, probe it for freezes (see
//! `insight`), and start it again after a crash.

use std::{
    collections::{HashMap, VecDeque},
    sync::{
        Arc, MutexGuard,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use tokio::sync::mpsc;
use tracing::{info, warn};

use super::{
    Lifecycle, Phase, Servers, console,
    insight::{self, Insight},
    records::{self, Outcome},
    restarts,
};
use crate::access::now;

/// Crashes within [`CRASH_WINDOW`] after which Core leaves a server stopped.
pub const CRASH_LIMIT: i64 = 3;
pub const CRASH_WINDOW: Duration = Duration::from_secs(10 * 60);
/// How long Core waits before starting a crashed server again.
const CRASH_DELAY: Duration = Duration::from_secs(5);
/// Probe words a late answer may still carry.
const WORDS_KEPT: usize = 4;

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
        if records::last_outcome(&self.db, id).await.ok().flatten()
            == Some(Outcome::Crashed)
        {
            self.after_crash(id).await;
        }
    }

    fn watched(&self) -> MutexGuard<'_, HashMap<String, Watched>> {
        self.insight.lock().expect("not poisoned")
    }

    /// What a running server's console said, `None` when it does not run.
    pub fn insight(&self, id: &str) -> Option<Insight> {
        self.watched()
            .get(id)
            .map(|watched| watched.insight.clone())
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

    /// Start a server that crashed again, unless crash restart is off for it or it crashed
    /// [`CRASH_LIMIT`] times in [`CRASH_WINDOW`]. Then it stays stopped, with its crash report
    /// in its runs.
    async fn after_crash(&self, id: &str) {
        if !restarts::get(&self.db, id)
            .await
            .is_ok_and(|restarts| restarts.crash)
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
        let Some(me) = self.me.upgrade() else { return };
        let id = id.to_owned();
        tokio::spawn(async move {
            tokio::time::sleep(CRASH_DELAY).await;
            info!(server = %id, "starting again after a crash");
            if let Err(error) = me.start(&id).await {
                warn!(server = %id, %error, "could not start again after a crash");
            }
        });
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
        let mut words: VecDeque<String> = VecDeque::new();
        loop {
            let ask = tokio::select! {
                line = lines.recv() => {
                    let Some(line) = line else { return };
                    let mut watched = self.watched();
                    let Some(this) = watched.get_mut(&id).filter(|this| this.run == run) else {
                        return;
                    };
                    let was_ready = this.insight.ready;
                    this.insight.read(&line);
                    if Insight::answers(&line, words.make_contiguous()) {
                        this.insight.answered_at = Some(now());
                        this.insight.frozen = false;
                    }
                    (!was_ready && this.insight.ready).then(|| "list".to_owned())
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
                    this.insight.ready.then(|| {
                        let word = insight::word();
                        words.push_back(word.clone());
                        if words.len() > WORDS_KEPT {
                            words.pop_front();
                        }
                        insight::probe(&word)
                    })
                }
                _ = list.tick() => {
                    self.insight(&id).filter(|insight| insight.ready).map(|_| "list".to_owned())
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

    async fn crashed(db: &sqlx::SqlitePool, server: &str, ended_at: i64) {
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
    }

    #[tokio::test]
    async fn a_server_that_keeps_crashing_is_left_stopped() {
        let dir = tempfile::tempdir().unwrap();
        let paths = Paths::new(dir.path());
        crate::daemon::folders::create(&paths, None).unwrap();
        let db = db::memory().await;
        let servers = Servers::open(paths, db.clone()).await.unwrap();
        let server = servers.create("A").await.unwrap().id;
        // An old crash does not count.
        crashed(&db, &server, now() - 3600).await;
        crashed(&db, &server, now() - 120).await;
        crashed(&db, &server, now() - 60).await;
        servers.after_crash(&server).await;
        assert!(!servers.gave_up(&server), "two recent crashes start again");
        crashed(&db, &server, now()).await;
        servers.after_crash(&server).await;
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
            crashed(&db, &other, now() - ago).await;
        }
        servers.after_crash(&other).await;
        assert!(!servers.gave_up(&other));
    }
}
