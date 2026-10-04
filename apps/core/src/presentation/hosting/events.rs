//! Hosting's node WebSocket and panel SSE protocols, backed by Core events.
use std::{
    convert::Infallible,
    sync::Arc,
    time::{Duration, Instant, SystemTime},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{HeaderMap, StatusCode},
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        Response,
    },
    routing::{get, post},
    Extension, Router,
};
use dashmap::DashMap;
use futures::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use sysinfo::{Pid, ProcessesToUpdate, System};
use tokio::{
    io::{AsyncReadExt, AsyncSeekExt},
    sync::broadcast,
};
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    application::{instance_status_service::send_command, state::AppState},
    domain::{
        event::Event,
        instance::{
            InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus,
        },
    },
    presentation::{
        error::ApiError, extractors::AuthUser,
        instance_path::resolve_authorized_instance,
    },
};

#[derive(Clone)]
struct LogCursor {
    offset: u64,
    created: Option<SystemTime>,
    process_started: Option<tokio::time::Instant>,
}

type LogCursors = Arc<DashMap<InstanceId, LogCursor>>;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/servers/:id/ws", get(upgrade))
        .route("/v1/sync", get(sync))
        .route("/servers/:id/v1/logs/clear", post(clear_logs))
        .layer(Extension(LogCursors::default()))
}

async fn clear_logs(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(cursors): Extension<LogCursors>,
) -> Result<StatusCode, ApiError> {
    require_dev(&state)?;
    let record =
        resolve_authorized_instance(&state, &claims.sub, &id, "server:logs")
            .await?;
    let path = std::path::Path::new(&record.data_dir).join("logs/latest.log");
    let metadata = match tokio::fs::metadata(path).await {
        Ok(metadata) => Some(metadata),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => return Err(ApiError::Internal(error.to_string())),
    };
    let process_started = state
        .instances
        .get(&record.id)
        .map(|handle| handle.started_at);
    cursors.insert(
        record.id,
        LogCursor {
            offset: metadata.as_ref().map_or(0, |metadata| metadata.len()),
            created: metadata.and_then(|metadata| metadata.created().ok()),
            process_started,
        },
    );
    Ok(StatusCode::NO_CONTENT)
}

fn require_dev(state: &AppState) -> Result<(), ApiError> {
    if state.config.no_auth && state.config.dev_mode {
        Ok(())
    } else {
        Err(ApiError::NotFound("development hosting is disabled".into()))
    }
}

async fn upgrade(
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Extension(cursors): Extension<LogCursors>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    require_dev(&state)?;
    let iid: InstanceId = id
        .parse()
        .map_err(|_| ApiError::NotFound("server not found".into()))?;
    state.instance_store.get(&iid).await?;
    Ok(ws
        .max_message_size(64 * 1024)
        .on_upgrade(move |socket| serve(socket, state, iid, cursors)))
}

#[derive(Deserialize)]
#[serde(tag = "event", rename_all = "kebab-case")]
enum ClientMessage {
    Auth { jwt: String },
    Command { cmd: String },
}

fn authenticate(state: &AppState, iid: &InstanceId, token: &str) -> bool {
    // Tickets are deliberately scoped to the URL's server and consumed once.
    state.ws_tickets.remove(token).is_some_and(|(_, ticket)| {
        ticket.expires_at > Instant::now()
            && ticket.user_id == format!("dev:{iid}")
    })
}

async fn send(socket: &mut WebSocket, data: Value) -> bool {
    socket.send(Message::Text(data.to_string())).await.is_ok()
}

async fn serve(
    mut socket: WebSocket,
    state: Arc<AppState>,
    iid: InstanceId,
    cursors: LogCursors,
) {
    // The generic client refreshes an expired/consumed ticket asynchronously
    // after auth-incorrect. Keep this bounded handshake open for that refresh.
    let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
    loop {
        let Ok(Some(Ok(Message::Text(text)))) =
            tokio::time::timeout_at(deadline, socket.recv()).await
        else {
            return;
        };
        let valid = match serde_json::from_str::<ClientMessage>(&text) {
            Ok(ClientMessage::Auth { jwt }) => authenticate(&state, &iid, &jwt),
            _ => false,
        };
        if valid {
            break;
        }
        if !send(&mut socket, json!({"event": "auth-incorrect"})).await {
            return;
        }
    }
    let mut events = state.broadcaster.subscribe();
    if !send(&mut socket, json!({"event": "auth-ok"})).await {
        return;
    }
    let Ok(mut record) = state.instance_store.get(&iid).await else {
        return;
    };
    if !send_state(&mut socket, &state, &record, None, None).await {
        return;
    }
    if !send_install(&mut socket, &record, None, None).await {
        return;
    }
    let cursor = cursors.get(&iid).map(|cursor| cursor.clone());
    let process_started =
        state.instances.get(&iid).map(|handle| handle.started_at);
    if !replay_log(&mut socket, &record, cursor, process_started).await {
        return;
    }

    let mut tick = tokio::time::interval(Duration::from_secs(3));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut system = System::new();
    let mut disk_sample = None;
    let mut disk_sampled_at = None;
    let mut progress = None;
    let mut install_error: Option<String> = None;
    loop {
        tokio::select! {
            _ = tick.tick() => {
                let Ok(latest) = state.instance_store.get(&iid).await else { break; };
                record = latest;
                if !send_state(&mut socket, &state, &record, progress, install_error.as_deref()).await { break; }
                let pid = state.instances.get(&iid).and_then(|handle| handle.pid);
                let dir = record.data_dir.clone();
                let refresh_disk = disk_sampled_at.is_none_or(|at: Instant| at.elapsed() >= Duration::from_secs(30));
                let sampled = tokio::task::spawn_blocking(move || {
                    let process = pid.and_then(|pid| {
                        let pid = Pid::from(pid as usize);
                        system.refresh_processes(ProcessesToUpdate::Some(&[pid]), true);
                        system.process(pid).map(|process| (process.cpu_usage(), process.memory()))
                    });
                    let disk = if refresh_disk { sample_disk(&dir) } else { None };
                    (system, process, disk)
                }).await;
                let Ok((updated_system, process, disk)) = sampled else { break; };
                system = updated_system;
                if refresh_disk {
                    disk_sample = disk;
                    disk_sampled_at = Some(Instant::now());
                }
                // A stopped process uses no CPU/RAM. Missing running-process samples
                // are withheld rather than represented as a successful zero reading.
                let process = process.or_else(|| if pid.is_none() { Some((0.0, 0)) } else { None });
                if let (Some((cpu, memory)), Some((used, total))) = (process, disk_sample) {
                    if !send(&mut socket, json!({
                        "event": "stats", "cpu_percent": cpu,
                        "ram_usage_bytes": memory, "ram_total_bytes": u64::from(record.memory.max_mb) * 1_048_576,
                        "storage_usage_bytes": used, "storage_total_bytes": total,
                    })).await { break; }
                }
            }
            event = events.recv() => {
                match event {
                    Ok(Event::InstanceOutput { instance_id, line }) if instance_id == iid => {
                        if !send(&mut socket, json!({"event": "log", "stream": "stdout", "message": line})).await { break; }
                    }
                    Ok(Event::StatusChanged { instance_id, status }) if instance_id == iid => {
                        record.status = status;
                        if !send_state(&mut socket, &state, &record, progress, install_error.as_deref()).await { break; }
                    }
                    Ok(Event::CreationProgress { instance_id, progress: value, .. }) if instance_id == iid => {
                        progress = Some(value.clamp(0.0, 1.0));
                        if !send_install(&mut socket, &record, progress, None).await { break; }
                        if !send_state(&mut socket, &state, &record, progress, install_error.as_deref()).await { break; }
                    }
                    Ok(Event::InstallStatusChanged { instance_id, install_status, message }) if instance_id == iid => {
                        record.install_status = install_status;
                        progress = None;
                        install_error = if record.install_status == InstanceInstallStatus::Failed { message } else { None };
                        if !send_install(&mut socket, &record, None, install_error.as_deref()).await { break; }
                        if record.install_status != InstanceInstallStatus::Installing {
                            let result = if record.install_status == InstanceInstallStatus::Ready { "ok" } else { "err" };
                            if !send(&mut socket, json!({"event": "installation-result", "result": result, "reason": install_error})).await { break; }
                        }
                        if !send_state(&mut socket, &state, &record, None, install_error.as_deref()).await { break; }
                    }
                    Ok(Event::InstanceUpdated { instance }) if instance.id == iid => { record = instance; }
                    Ok(Event::InstanceDeleted { instance_id }) if instance_id == iid => {
                        cursors.remove(&iid);
                        break;
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {
                        if !send(&mut socket, json!({"event": "log", "stream": "stderr", "message": "Some live output was missed. The complete server log is available in Files under logs/."})).await { break; }
                    }
                    _ => {}
                }
            }
            message = socket.recv() => {
                match message {
                    Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
                        Ok(ClientMessage::Command { cmd }) => {
                            if let Err(error) = send_command(&state, &iid, cmd).await {
                                if !send(&mut socket, json!({"event": "log", "stream": "stderr", "message": error.to_string()})).await { break; }
                            }
                        }
                        Ok(ClientMessage::Auth { jwt }) => {
                            if !authenticate(&state, &iid, &jwt) {
                                send(&mut socket, json!({"event": "auth-incorrect"})).await;
                                break;
                            }
                            if !send(&mut socket, json!({"event": "auth-ok"})).await { break; }
                        }
                        Err(_) => {
                            if !send(&mut socket, json!({"event": "log", "stream": "stderr", "message": "Invalid console message"})).await { break; }
                        }
                    },
                    Some(Ok(Message::Ping(data))) => { if socket.send(Message::Pong(data)).await.is_err() { break; } }
                    Some(Ok(Message::Close(_))) => {
                        // Flush the queued close reply before dropping the socket.
                        let _ = socket.flush().await;
                        break;
                    }
                    None | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
    let _ = socket.close().await;
}

async fn send_state(
    socket: &mut WebSocket,
    state: &AppState,
    record: &InstanceRecord,
    progress: Option<f32>,
    error: Option<&str>,
) -> bool {
    let uptime = state
        .instances
        .get(&record.id)
        .map(|handle| handle.started_at.elapsed().as_secs())
        .unwrap_or(0);
    let power = match record.status {
        InstanceStatus::Offline => "stopped",
        InstanceStatus::Starting => "starting",
        InstanceStatus::Running => "running",
        InstanceStatus::Stopping => "stopping",
        InstanceStatus::Crashed => "crashed",
    };
    let variant = if record.install_status != InstanceInstallStatus::Ready {
        "not_ready"
    } else if matches!(
        record.status,
        InstanceStatus::Offline | InstanceStatus::Crashed
    ) {
        "idle"
    } else {
        power
    };
    let progress = progress.map(|percent| json!({"started_at": record.updated_at, "phase": "InstallingLoader", "percent": percent * 100.0}));
    let error = if record.install_status == InstanceInstallStatus::Failed {
        Some(
            json!({"step": "installation", "description": error.unwrap_or("Server installation failed. Repair the installation to retry.")}),
        )
    } else {
        None
    };
    send(socket, json!({"event": "state", "debug": record.install_status.to_string(), "power_variant": variant,
        "target": null, "uptime": uptime, "progress": progress, "content_error": error})).await
        && send(socket, json!({"event": "power-state", "state": power})).await
}

async fn send_install(
    socket: &mut WebSocket,
    record: &InstanceRecord,
    progress: Option<f32>,
    error: Option<&str>,
) -> bool {
    let error = if record.install_status == InstanceInstallStatus::Failed {
        Some(error.unwrap_or(
            "Server installation failed. Repair the installation to retry.",
        ))
    } else {
        error
    };
    let items = if record.install_status == InstanceInstallStatus::Ready {
        vec![]
    } else {
        vec![
            json!({"world_id": record.id, "id": format!("installation:{}", record.id),
            "key": {"type": "platform", "platform": record.loader.to_string(), "platform_version": record.loader_version.as_deref().unwrap_or(""), "game_version": record.game_version},
            "progress": progress, "error": error}),
        ]
    };
    send(socket, json!({"event": "install-progress", "items": items})).await
}

async fn replay_log(
    socket: &mut WebSocket,
    record: &InstanceRecord,
    cursor: Option<LogCursor>,
    process_started: Option<tokio::time::Instant>,
) -> bool {
    let path = std::path::Path::new(&record.data_dir).join("logs/latest.log");
    let Ok(mut file) = tokio::fs::File::open(path).await else {
        return true;
    };
    let Ok(metadata) = file.metadata().await else {
        return true;
    };
    let cleared_offset = cursor
        .filter(|cursor| {
            cursor.process_started == process_started
                && cursor.created == metadata.created().ok()
                && cursor.offset <= metadata.len()
        })
        .map_or(0, |cursor| cursor.offset);
    let start = metadata.len().saturating_sub(64 * 1024).max(cleared_offset);
    if file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return true;
    }
    let mut bytes = Vec::new();
    if file.take(64 * 1024).read_to_end(&mut bytes).await.is_err() {
        return true;
    }
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines();
    if start > cleared_offset {
        lines.next();
    }
    for line in lines {
        if !send(
            socket,
            json!({"event": "log", "stream": "stdout", "message": line}),
        )
        .await
        {
            return false;
        }
    }
    true
}

fn sample_disk(dir: &str) -> Option<(u64, u64)> {
    let total = fs3::total_space(dir).ok()?;
    let mut used = 0u64;
    for entry in walkdir::WalkDir::new(dir).follow_links(false) {
        let entry = entry.ok()?;
        if entry.file_type().is_file() {
            used = used.saturating_add(entry.metadata().ok()?.len());
        }
    }
    Some((used, total))
}

#[derive(Deserialize)]
struct SyncQuery {
    scope: String,
    intent: Option<String>,
}

async fn sync(
    State(state): State<Arc<AppState>>,
    Query(query): Query<SyncQuery>,
    headers: HeaderMap,
) -> Result<
    Sse<impl futures::Stream<Item = Result<SseEvent, Infallible>>>,
    ApiError,
> {
    require_dev(&state)?;
    let id = query.scope.strip_prefix("server:").ok_or_else(|| {
        ApiError::BadRequest("scope must be server:<id>".into())
    })?;
    let iid: InstanceId = id
        .parse()
        .map_err(|_| ApiError::NotFound("server not found".into()))?;
    let record = state.instance_store.get(&iid).await?;
    if let Some(intent) = &query.intent {
        if !intent.split(',').all(|part| {
            matches!(
                part,
                "all" | "server" | "world" | "backup" | "users" | "protocol"
            )
        }) {
            return Err(ApiError::BadRequest("invalid sync intent".into()));
        }
    }
    // Core has no durable Hosting event cursor. A reconnect with a cursor must
    // refetch, rather than silently dropping mutations while disconnected.
    let initial = if headers.contains_key("last-event-id") {
        SseEvent::default().data(json!({"type": "protocol.reset"}).to_string())
    } else {
        // This real snapshot also establishes a cursor so reconnects invalidate
        // panel data after a transport interruption.
        SseEvent::default().id(uuid::Uuid::new_v4().to_string()).data(json!({
            "type": "server.network.patch", "ports": [{"port": record.port, "name": "Minecraft"}]
        }).to_string())
    };
    let changes = BroadcastStream::new(state.broadcaster.subscribe())
        .filter_map(move |event| {
            let reset = match event {
                Ok(Event::InstanceUpdated { instance }) => instance.id == iid,
                Ok(Event::InstanceDeleted { instance_id })
                | Ok(Event::InstallStatusChanged { instance_id, .. })
                | Ok(Event::FsChanged { instance_id, .. }) => {
                    instance_id == iid
                }
                Err(_) => true,
                _ => false,
            };
            futures::future::ready(reset.then(|| {
                SseEvent::default()
                    .data(json!({"type": "protocol.reset"}).to_string())
            }))
        });
    let stream = futures::stream::once(futures::future::ready(initial))
        .chain(changes)
        .map(Ok);
    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
