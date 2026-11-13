//! `GET /events`: one socket per client, on which it subscribes to topics
//! (`contracts::core::Topic`) and gets the Core's [`CoreEvent`]s and the engine's events for them.
//! Every topic is checked against the caller's role when subscribed, and again when their access
//! changes, when the socket falls behind, and every [`RECHECK_EVERY`]: a topic they lost ends with
//! [`CoreEvent::Ended`], and a session that ended closes the socket.

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
use engine::Event;
use tokio::sync::{
    broadcast::{self, error::RecvError},
    mpsc,
};

use super::{CoreState, servers::power_state};
use crate::{
    access::{Caller, Changed, Permission, StreamGuard, now},
    servers::{Phase, console, restarts::run_started_at},
};

/// Console lines a new console subscription starts with.
const BACKLOG: usize = 200;
/// How often an open socket checks that its session has not expired.
const RECHECK_EVERY: Duration = Duration::from_secs(600);
/// How often the Core looks for power changes. A server's phase changes in many places, so one
/// task reads them all instead of each of them announcing it.
const POWER_EVERY: Duration = Duration::from_millis(500);
/// Console lines a subscriber may fall behind by before it lags.
const CONSOLE_CAPACITY: usize = 256;
/// How often a console follower checks whether anyone still reads it.
const FOLLOWER_IDLE_CHECK: Duration = Duration::from_secs(5);

pub async fn socket(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    // Before any role check, so a change from here on is never missed.
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
            visible: HashMap::new(),
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
    /// Whether the caller may see each instance an install job names, until the next re-check.
    visible: HashMap<String, bool>,
    /// Where console subscriptions forward their lines, or `Resync` when they fell behind.
    console: mpsc::Sender<CoreEvent>,
}

impl Loop {
    async fn run(
        mut self,
        mut socket: WebSocket,
        mut changes: broadcast::Receiver<Changed>,
        mut core: broadcast::Receiver<CoreEvent>,
        mut engine: broadcast::Receiver<Event>,
        mut consoles: mpsc::Receiver<CoreEvent>,
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
        consoles: &mut mpsc::Receiver<CoreEvent>,
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
                        if self.wants_engine(&event).await {
                            send(socket, &event).await?;
                        }
                        false
                    }
                    Err(RecvError::Lagged(_)) => true,
                    Err(RecvError::Closed) => return Err(Closed),
                },
                Some(event) = consoles.recv() => match event {
                    CoreEvent::Resync => true,
                    // A line forwarded before its topic ended is dropped.
                    CoreEvent::Console { ref server, .. }
                        if !self.topics.contains_key(&Topic::Console { id: server.clone() }) => false,
                    event => {
                        send(socket, &event).await?;
                        false
                    }
                },
                change = changes.recv() => {
                    let concerns_me = match change {
                        Ok(Changed::User(user)) => self.caller.user_id() == Some(user.as_str()),
                        Ok(Changed::Everyone) | Err(_) => true,
                    };
                    if concerns_me {
                        self.recheck(socket).await?;
                    }
                    false
                }
                _ = recheck.tick() => {
                    self.recheck(socket).await?;
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
                self.recheck(socket).await?;
                send(socket, &CoreEvent::Resync).await?;
                let servers: Vec<String> = self
                    .topics
                    .keys()
                    .filter_map(|topic| match topic {
                        Topic::Server { id } => Some(id.clone()),
                        _ => None,
                    })
                    .collect();
                for server in servers {
                    self.snapshot(socket, &server).await?;
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
        if let Err(error) = self.allowed(&topic).await {
            return send(socket, &CoreEvent::Ended { topic, error }).await;
        }
        let forward = match &topic {
            Topic::Console { id } => {
                let path = self.state.servers.paths().console_log(id);
                let (backlog, lines) = self.state.consoles.subscribe(path, id);
                for line in backlog {
                    let line = CoreEvent::Console {
                        server: id.clone(),
                        line,
                    };
                    send(socket, &line).await?;
                }
                Some(forward(id.clone(), lines, self.console.clone()))
            }
            _ => None,
        };
        let server = match &topic {
            Topic::Server { id } => Some(id.clone()),
            _ => None,
        };
        self.topics.insert(topic, forward);
        match server {
            Some(server) => self.snapshot(socket, &server).await,
            None => Ok(()),
        }
    }

    /// Whether the caller may subscribe to `topic`. Install jobs are filtered one by one.
    async fn allowed(&self, topic: &Topic) -> Result<(), ApiError> {
        let (id, permission) = match topic {
            Topic::Installs => return Ok(()),
            Topic::Console { id } => (id, Permission::Edit),
            Topic::Instance { id }
            | Topic::Server { id }
            | Topic::Backups { id } => (id, Permission::View),
        };
        self.state
            .access
            .require(&self.caller, id, permission)
            .await
            .map(drop)
    }

    /// Close on an ended session; end each topic the caller lost.
    async fn recheck(&mut self, socket: &mut WebSocket) -> Result<(), Closed> {
        if !self.state.access.revalidate(&self.caller).await {
            return Err(Closed);
        }
        self.visible.clear();
        let topics: Vec<Topic> = self.topics.keys().cloned().collect();
        for topic in topics {
            if let Err(error) = self.allowed(&topic).await {
                self.topics.remove(&topic);
                send(socket, &CoreEvent::Ended { topic, error }).await?;
            }
        }
        Ok(())
    }

    /// The server's power, as a `Server` subscription starts with.
    async fn snapshot(
        &self,
        socket: &mut WebSocket,
        server: &str,
    ) -> Result<(), Closed> {
        let phase = self.state.servers.phase(server);
        let power = power_of(&self.state, server, phase)
            .await
            .unwrap_or(PowerState::Stopped);
        send(socket, &power_event(&self.state, server, power).await).await
    }

    fn wants(&self, event: &CoreEvent) -> bool {
        let topic = match event {
            CoreEvent::Power { server, .. } => {
                Topic::Server { id: server.clone() }
            }
            CoreEvent::Backup { server, .. } => {
                Topic::Backups { id: server.clone() }
            }
            CoreEvent::Console { server, .. } => {
                Topic::Console { id: server.clone() }
            }
            CoreEvent::Ended { .. } | CoreEvent::Resync => return false,
        };
        self.topics.contains_key(&topic)
    }

    async fn wants_engine(&mut self, event: &Event) -> bool {
        let instance = match event {
            Event::Instance(payload) => &payload.instance_id,
            Event::Process(payload) => &payload.instance_id,
            Event::InstallJob(job) => {
                if !self.topics.contains_key(&Topic::Installs) {
                    return false;
                }
                let Some(instance) = &job.instance_id else {
                    return self.caller.is_owner();
                };
                if let Some(&visible) = self.visible.get(instance) {
                    return visible;
                }
                let visible = self
                    .state
                    .access
                    .require(&self.caller, instance, Permission::View)
                    .await
                    .is_ok();
                self.visible.insert(instance.clone(), visible);
                return visible;
            }
            _ => return false,
        };
        self.topics.contains_key(&Topic::Instance {
            id: instance.clone(),
        })
    }
}

/// Pass a console subscription's lines to its socket, or `Resync` when it fell behind.
fn forward(
    server: String,
    mut lines: broadcast::Receiver<String>,
    out: mpsc::Sender<CoreEvent>,
) -> Forward {
    Forward(tokio::spawn(async move {
        loop {
            let event = match lines.recv().await {
                Ok(line) => CoreEvent::Console {
                    server: server.clone(),
                    line,
                },
                Err(RecvError::Lagged(_)) => CoreEvent::Resync,
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

/// Send a `Power` event whenever a server's phase changes, for every socket subscribed to it.
pub async fn watch_power(state: Arc<CoreState>) {
    let mut seen: HashMap<String, Phase> = HashMap::new();
    let mut tick = tokio::time::interval(POWER_EVERY);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        tick.tick().await;
        let phases = state.servers.phases();
        if state.backups.events.receiver_count() == 0 {
            seen = phases;
            continue;
        }
        let ended: Vec<String> = seen
            .keys()
            .filter(|id| !phases.contains_key(*id))
            .cloned()
            .collect();
        for (id, phase) in phases {
            if seen.get(&id) != Some(&phase)
                && announce(&state, &id, Some(phase)).await
            {
                seen.insert(id, phase);
            }
        }
        for id in ended {
            if announce(&state, &id, None).await {
                seen.remove(&id);
            }
        }
    }
}

/// Send `id`'s power at `phase`. False when it could not be read, to try again.
async fn announce(state: &CoreState, id: &str, phase: Option<Phase>) -> bool {
    let Some(power) = power_of(state, id, phase).await else {
        return false;
    };
    let _ = state
        .backups
        .events
        .send(power_event(state, id, power).await);
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
async fn power_event(
    state: &CoreState,
    id: &str,
    power: PowerState,
) -> CoreEvent {
    let uptime = match power {
        PowerState::Running => run_started_at(&state.db, id)
            .await
            .map(|started| u64::try_from(now() - started).unwrap_or(0)),
        _ => None,
    };
    CoreEvent::Power {
        server: id.to_owned(),
        state: power,
        uptime,
    }
}
