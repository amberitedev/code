//! `GET /events`: one socket per client, on which it subscribes to topics
//! (`contracts::core::Topic`) and gets the Core's [`CoreEvent`]s and the engine's events for them.
//! Only the owner opens one, and the owner sees every topic. The session is checked again when it
//! changes, when the socket falls behind, and every [`RECHECK_EVERY`]; one that ended closes the
//! socket.

use std::{
    collections::{HashMap, VecDeque},
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    extract::{
        State,
        ws::{Message, WebSocket, WebSocketUpgrade},
    },
    response::{IntoResponse, Response},
};
pub use contracts::core::{
    BackupTask, BackupTaskState as BackupState, CoreEvent,
};
use contracts::{
    ApiError, ErrorCode,
    core::{ClientMessage, PowerState, ServerPower, Topic},
};
use engine::{Event, LogEvent, LogPayload};
use tokio::sync::{
    broadcast::{self, error::RecvError},
    mpsc,
};

use super::{CoreState, servers::power_state};
use crate::{
    access::{Caller, Changed, StreamGuard, now, require_owner},
    servers::{Phase, console, restarts::run_started_at},
};

/// Console lines a new console subscription starts with.
const BACKLOG: usize = 200;
/// How often an open socket checks that its session has not expired.
const RECHECK_EVERY: Duration = Duration::from_secs(600);
/// How long a power announcement that could not read a server's last run waits to try again.
const POWER_RETRY: Duration = Duration::from_secs(1);
/// Console lines a subscriber may fall behind by before it lags.
const CONSOLE_CAPACITY: usize = 256;
/// How often a console follower checks whether anyone still reads it.
const FOLLOWER_IDLE_CHECK: Duration = Duration::from_secs(5);

pub async fn socket(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    if let Err(error) = require_owner(&caller) {
        return error.into_response();
    }
    // Before any access check, so a change from here on is never missed.
    let changes = state.access.changed.subscribe();
    let Some(guard) = state.access.open_stream(&caller) else {
        return ApiError::new(
            ErrorCode::RateLimited,
            "This session has too many event sockets open.",
        )
        .into_response();
    };
    let core = state.backups.events.subscribe();
    let engine = state.engine.events.subscribe();
    upgrade.on_upgrade(move |socket| {
        let (console, consoles) = mpsc::channel(CONSOLE_CAPACITY);
        Loop {
            state,
            caller,
            _guard: guard,
            topics: HashMap::new(),
            console,
        }
        .run(socket, changes, core, engine, consoles)
    })
}

/// A console subscription's forwarding task, stopped when dropped.
struct Forward(tokio::task::JoinHandle<()>);

impl Drop for Forward {
    fn drop(&mut self) {
        self.0.abort();
    }
}

/// The socket closed, or its session ended.
struct Closed;

struct Loop {
    state: Arc<CoreState>,
    caller: Caller,
    _guard: StreamGuard,
    /// Each subscribed topic, with a console subscription's forwarding task.
    topics: HashMap<Topic, Option<Forward>>,
    /// Where console subscriptions forward their lines.
    console: mpsc::Sender<Line>,
}

/// What a console subscription forwards.
enum Line {
    Line {
        server: String,
        line: String,
    },
    /// It fell behind and lost lines.
    Lagged,
}

/// A console line as the engine's `log` event, the same as an App instance's.
fn log_event(server: String, line: String) -> Event {
    Event::Log(LogPayload {
        instance_id: server,
        event: LogEvent::Legacy { message: line },
    })
}

impl Loop {
    async fn run(
        mut self,
        mut socket: WebSocket,
        mut changes: broadcast::Receiver<Changed>,
        mut core: broadcast::Receiver<CoreEvent>,
        mut engine: broadcast::Receiver<Event>,
        mut consoles: mpsc::Receiver<Line>,
    ) {
        let mut recheck = tokio::time::interval(RECHECK_EVERY);
        recheck.tick().await;
        let _ = self
            .serve(
                &mut socket,
                &mut changes,
                &mut core,
                &mut engine,
                &mut consoles,
                &mut recheck,
            )
            .await;
        let _ = socket.send(Message::Close(None)).await;
    }

    async fn serve(
        &mut self,
        socket: &mut WebSocket,
        changes: &mut broadcast::Receiver<Changed>,
        core: &mut broadcast::Receiver<CoreEvent>,
        engine: &mut broadcast::Receiver<Event>,
        consoles: &mut mpsc::Receiver<Line>,
        recheck: &mut tokio::time::Interval,
    ) -> Result<(), Closed> {
        loop {
            let lagged = tokio::select! {
                event = core.recv() => match event {
                    Ok(event) => {
                        if self.wants(&event) {
                            send(socket, &event).await?;
                        }
                        false
                    }
                    Err(RecvError::Lagged(_)) => true,
                    Err(RecvError::Closed) => return Err(Closed),
                },
                event = engine.recv() => match event {
                    Ok(event) => {
                        if self.wants_engine(&event) {
                            send(socket, &event).await?;
                        }
                        if let Some(changed) = self.server_changed(&event) {
                            send(socket, &changed).await?;
                        }
                        false
                    }
                    Err(RecvError::Lagged(_)) => true,
                    Err(RecvError::Closed) => return Err(Closed),
                },
                Some(line) = consoles.recv() => match line {
                    Line::Lagged => true,
                    // A line forwarded before its topic ended is dropped.
                    Line::Line { server, .. }
                        if !self.topics.contains_key(&Topic::Console { id: server.clone() }) => false,
                    Line::Line { server, line } => {
                        send(socket, &log_event(server, line)).await?;
                        false
                    }
                },
                change = changes.recv() => {
                    let concerns_me = match change {
                        Ok(Changed::User(user)) => self.caller.user_id() == Some(user.as_str()),
                        Ok(Changed::Everyone) | Err(_) => true,
                    };
                    if concerns_me {
                        self.recheck().await?;
                    }
                    false
                }
                _ = recheck.tick() => {
                    self.recheck().await?;
                    false
                }
                message = socket.recv() => match message {
                    Some(Ok(Message::Text(text))) => {
                        if let Ok(message) = serde_json::from_str(&text) {
                            self.handle(socket, message).await?;
                        }
                        false
                    }
                    Some(Ok(_)) => false,
                    Some(Err(_)) | None => return Err(Closed),
                },
            };
            if lagged {
                self.recheck().await?;
                send(socket, &CoreEvent::Resync).await?;
                let servers = self.watched_servers().await;
                for server in servers {
                    self.snapshot(socket, &server).await?;
                }
                if self.topics.contains_key(&Topic::Installs) {
                    self.unfinished_jobs(socket).await?;
                }
            }
        }
    }

    async fn handle(
        &mut self,
        socket: &mut WebSocket,
        message: ClientMessage,
    ) -> Result<(), Closed> {
        let topic = match message {
            ClientMessage::Unsubscribe { topic } => {
                self.topics.remove(&topic);
                return Ok(());
            }
            ClientMessage::Subscribe { topic } => topic,
        };
        if self.topics.contains_key(&topic) {
            return Ok(());
        }
        let forward = match &topic {
            Topic::Console { id } => {
                let path = self.state.servers.paths().console_log(id);
                let (backlog, lines) = self.state.consoles.subscribe(path, id);
                for line in backlog {
                    send(socket, &log_event(id.clone(), line)).await?;
                }
                Some(forward(id.clone(), lines, self.console.clone()))
            }
            _ => None,
        };
        let start = topic.clone();
        self.topics.insert(topic, forward);
        match start {
            Topic::Server { id } => self.snapshot(socket, &id).await,
            Topic::Servers => {
                for id in self.all_servers().await {
                    self.snapshot(socket, &id).await?;
                }
                Ok(())
            }
            Topic::Installs => self.unfinished_jobs(socket).await,
            _ => Ok(()),
        }
    }

    /// Close once the session ended.
    async fn recheck(&self) -> Result<(), Closed> {
        match self.state.access.revalidate(&self.caller).await {
            true => Ok(()),
            false => Err(Closed),
        }
    }

    /// The server's power, as a `Server` subscription starts with. A server that does not exist
    /// has none.
    async fn snapshot(
        &self,
        socket: &mut WebSocket,
        server: &str,
    ) -> Result<(), Closed> {
        let exists = crate::servers::server_rows::ids(&self.state.db)
            .await
            .is_ok_and(|ids| ids.iter().any(|id| id == server));
        let phase = self.state.servers.phase(server);
        let Some(power) = power_of(&self.state, server, phase)
            .await
            .filter(|_| exists)
        else {
            return Ok(());
        };
        for event in power_events(&self.state, server, power).await {
            send(socket, &event).await?;
        }
        Ok(())
    }

    /// The unfinished install jobs, as an `Installs` subscription starts with.
    async fn unfinished_jobs(
        &self,
        socket: &mut WebSocket,
    ) -> Result<(), Closed> {
        let jobs = engine::install::list_jobs(false, &self.state.engine)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "listing install jobs");
                Vec::new()
            });
        for job in jobs {
            if !job.status.is_finished() {
                send(socket, &Event::InstallJob(Arc::new(job))).await?;
            }
        }
        Ok(())
    }

    fn wants(&self, event: &CoreEvent) -> bool {
        let topic = match event {
            CoreEvent::PowerState { server, .. }
            | CoreEvent::Uptime { server, .. }
            | CoreEvent::ServerChanged { server } => {
                if self.topics.contains_key(&Topic::Servers) {
                    return true;
                }
                Topic::Server { id: server.clone() }
            }
            CoreEvent::BackupProgress { server, .. } => {
                Topic::Backups { id: server.clone() }
            }
            // The caller that started it may be on any socket.
            CoreEvent::OpEnded { .. } => return true,
            CoreEvent::Resync => return false,
        };
        self.topics.contains_key(&topic)
    }

    /// A server's instance was made, edited, removed or had content installed: what its `Server`
    /// shows may have changed, for whoever watches it.
    fn server_changed(&self, event: &Event) -> Option<CoreEvent> {
        let Event::Instance(payload) = event else {
            return None;
        };
        use engine::InstancePayloadType::*;
        if !matches!(
            payload.event,
            Created
                | Edited
                | Removed
                | ContentInstallFinished { .. }
                | ContentInstallFailed { .. }
        ) {
            return None;
        }
        let changed = CoreEvent::ServerChanged {
            server: payload.instance_id.clone(),
        };
        self.wants(&changed).then_some(changed)
    }

    /// Every server on the Core.
    async fn all_servers(&self) -> Vec<String> {
        crate::servers::server_rows::ids(&self.state.db)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "listing servers");
                Vec::new()
            })
    }

    /// The servers a subscription asks for the power of.
    async fn watched_servers(&self) -> Vec<String> {
        if self.topics.contains_key(&Topic::Servers) {
            return self.all_servers().await;
        }
        self.topics
            .keys()
            .filter_map(|topic| match topic {
                Topic::Server { id } => Some(id.clone()),
                _ => None,
            })
            .collect()
    }

    fn wants_engine(&self, event: &Event) -> bool {
        let instance = match event {
            Event::Instance(payload) => &payload.instance_id,
            Event::Process(payload) => &payload.instance_id,
            Event::InstallJob(_) => {
                return self.topics.contains_key(&Topic::Installs);
            }
            _ => return false,
        };
        self.topics.contains_key(&Topic::Instance {
            id: instance.clone(),
        })
    }
}

/// Pass a console subscription's lines to its socket, and say when it fell behind.
fn forward(
    server: String,
    mut lines: broadcast::Receiver<String>,
    out: mpsc::Sender<Line>,
) -> Forward {
    Forward(tokio::spawn(async move {
        loop {
            let event = match lines.recv().await {
                Ok(line) => Line::Line {
                    server: server.clone(),
                    line,
                },
                Err(RecvError::Lagged(_)) => Line::Lagged,
                Err(RecvError::Closed) => return,
            };
            if out.send(event).await.is_err() {
                return;
            }
        }
    }))
}

async fn send(
    socket: &mut WebSocket,
    event: &impl serde::Serialize,
) -> Result<(), Closed> {
    let text = serde_json::to_string(event).expect("events serialize");
    socket
        .send(Message::Text(text.into()))
        .await
        .map_err(|_| Closed)
}

/// One follower per server's console, shared by every socket subscribed to it.
#[derive(Default)]
pub struct Consoles(Mutex<HashMap<String, Arc<Mutex<Feed>>>>);

struct Feed {
    /// The last lines for people. Locked with `lines`, so a new subscriber's backlog ends where
    /// its receiver starts.
    backlog: VecDeque<String>,
    lines: broadcast::Sender<String>,
}

impl Consoles {
    /// The last [`BACKLOG`] lines for people of the console log at `path`, and every new one.
    fn subscribe(
        self: &Arc<Self>,
        path: std::path::PathBuf,
        server: &str,
    ) -> (Vec<String>, broadcast::Receiver<String>) {
        let mut feeds = self.0.lock().expect("not poisoned");
        let feed = feeds
            .entry(server.to_owned())
            .or_insert_with(|| self.follow(path, server));
        let feed = feed.lock().expect("not poisoned");
        (
            feed.backlog.iter().cloned().collect(),
            feed.lines.subscribe(),
        )
    }

    fn follow(
        self: &Arc<Self>,
        path: std::path::PathBuf,
        server: &str,
    ) -> Arc<Mutex<Feed>> {
        let (backlog, end) = console::tail(&path, BACKLOG).unwrap_or_default();
        let feed = Arc::new(Mutex::new(Feed {
            backlog: backlog
                .into_iter()
                .filter(|line| console::for_people(line))
                .collect(),
            lines: broadcast::channel(CONSOLE_CAPACITY).0,
        }));
        let (sender, mut lines) = mpsc::channel(CONSOLE_CAPACITY);
        console::follow(path, end, sender);
        let (consoles, server, followed) =
            (Arc::clone(self), server.to_owned(), Arc::clone(&feed));
        tokio::spawn(async move {
            let mut idle = tokio::time::interval(FOLLOWER_IDLE_CHECK);
            loop {
                tokio::select! {
                    line = lines.recv() => {
                        let Some(line) = line else { break };
                        if !console::for_people(&line) {
                            continue;
                        }
                        let mut feed = followed.lock().expect("not poisoned");
                        if feed.backlog.len() == BACKLOG {
                            feed.backlog.pop_front();
                        }
                        feed.backlog.push_back(line.clone());
                        let _ = feed.lines.send(line);
                    }
                    _ = idle.tick() => {
                        let mut feeds = consoles.0.lock().expect("not poisoned");
                        if followed.lock().expect("not poisoned").lines.receiver_count() == 0 {
                            feeds.remove(&server);
                            return;
                        }
                    }
                }
            }
            let mut feeds = consoles.0.lock().expect("not poisoned");
            if feeds
                .get(&server)
                .is_some_and(|feed| Arc::ptr_eq(feed, &followed))
            {
                feeds.remove(&server);
            }
        });
        feed
    }
}

/// Send a `Power` event whenever a server's phase changes, for every socket subscribed to it. A
/// server's phase changes in many places, so this one task follows them all
/// ([`Servers::changes`](crate::servers::Servers::changes)) instead of each announcing it.
pub async fn watch_power(state: Arc<CoreState>) {
    let mut changes = state.servers.changes();
    let mut seen: HashMap<String, Phase> = HashMap::new();
    loop {
        let phases = state.servers.phases();
        let mut unread = false;
        if state.backups.events.receiver_count() == 0 {
            seen = phases;
        } else {
            let ended: Vec<String> = seen
                .keys()
                .filter(|id| !phases.contains_key(*id))
                .cloned()
                .collect();
            for (id, phase) in phases {
                if seen.get(&id) == Some(&phase) {
                    continue;
                }
                if announce(&state, &id, Some(phase)).await {
                    seen.insert(id, phase);
                } else {
                    unread = true;
                }
            }
            for id in ended {
                if announce(&state, &id, None).await {
                    seen.remove(&id);
                } else {
                    unread = true;
                }
            }
        }
        if unread {
            tokio::time::sleep(POWER_RETRY).await;
        } else if changes.changed().await.is_err() {
            return;
        }
    }
}

/// Send `id`'s power at `phase`. False when it could not be read, to try again.
async fn announce(state: &CoreState, id: &str, phase: Option<Phase>) -> bool {
    let Some(power) = power_of(state, id, phase).await else {
        return false;
    };
    for event in power_events(state, id, power).await {
        let _ = state.backups.events.send(event);
    }
    // A change of phase also changes `status`, such as the end of an install.
    let _ = state.backups.events.send(CoreEvent::ServerChanged {
        server: id.to_owned(),
    });
    true
}

/// `id`'s power state at `phase`, as the hosting API's socket names it, or `None` when its last
/// run could not be read. A killed server reads as stopped.
async fn power_of(
    state: &CoreState,
    id: &str,
    phase: Option<Phase>,
) -> Option<PowerState> {
    let power = match power_state(state, id, phase).await {
        Ok(power) => power,
        Err(error) => {
            tracing::debug!(server = %id, %error, "power state");
            return None;
        }
    };
    Some(match power {
        ServerPower::Starting => PowerState::Starting,
        ServerPower::Running => PowerState::Running,
        ServerPower::Stopping => PowerState::Stopping,
        ServerPower::Crashed => PowerState::Crashed,
        ServerPower::Offline | ServerPower::Killed => PowerState::Stopped,
    })
}

/// `power`, and for a running server how long its run has been up, which the App's uptime counts
/// on from.
async fn power_events(
    state: &CoreState,
    id: &str,
    power: PowerState,
) -> Vec<CoreEvent> {
    let uptime = match power {
        PowerState::Running => run_started_at(&state.db, id)
            .await
            .map(|started| u64::try_from(now() - started).unwrap_or(0)),
        _ => None,
    };
    let server = id.to_owned();
    let mut events = vec![CoreEvent::PowerState {
        server: server.clone(),
        state: power,
    }];
    events.extend(uptime.map(|uptime| CoreEvent::Uptime { server, uptime }));
    events
}
