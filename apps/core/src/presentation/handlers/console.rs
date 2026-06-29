use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    response::{
        sse::{Event as SseEvent, KeepAlive, Sse},
        Response,
    },
    Json,
};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use sysinfo::{Pid, System};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    application::{
        instance_status_service::send_command,
        state::{AppState, WsTicket},
    },
    domain::{event::Event, instance::InstanceId},
	presentation::{
		authz::require_instance_permission, error::ApiError,
		extractors::AuthUser,
		instance_path::resolve_authorized_instance_id,
	},
};

/// POST /ws-token — issue a 60-second WebSocket ticket (requires auth).
pub async fn issue_ws_token(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let ticket = uuid::Uuid::new_v4().to_string();
    state.ws_tickets.insert(
        ticket.clone(),
        WsTicket {
            user_id: claims.sub,
            expires_at: Instant::now() + Duration::from_secs(60),
        },
    );
    Json(json!({ "ticket": ticket }))
}

#[derive(Deserialize)]
pub struct WsQuery {
    pub ticket: String,
}

/// GET /instances/:id/console?ticket=<uuid> — WebSocket console.
pub async fn ws_console(
    Path(id): Path<String>,
    Query(q): Query<WsQuery>,
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
	let user_id = validate_ticket(&state, &q.ticket)?;

	// Allow connecting to any existing instance, even while stopped — the
	// socket then streams logs/state/stats once the instance starts. This
	// mirrors the upstream hosting console, which is always connectable.
	let iid = resolve_authorized_instance_id(
		&state,
		&user_id,
		&id,
		"server:console",
	)
	.await?;

    let rx = state.broadcaster.subscribe();
    let state_clone = Arc::clone(&state);
    Ok(ws.on_upgrade(move |socket| {
        ws_handler(socket, iid, user_id, state_clone, rx)
    }))
}

fn validate_ticket(state: &AppState, ticket: &str) -> Result<String, ApiError> {
    match state.ws_tickets.remove(ticket) {
        Some((_, t)) if t.expires_at > Instant::now() => Ok(t.user_id),
        Some(_) => Err(ApiError::Unauthorized("ticket expired".into())),
        None => Err(ApiError::Unauthorized("invalid ticket".into())),
    }
}

async fn ws_handler(
    mut socket: WebSocket,
    iid: InstanceId,
    user_id: String,
    state: Arc<AppState>,
    mut rx: broadcast::Receiver<Event>,
) {
    let mut stats_interval = tokio::time::interval(Duration::from_secs(3));
    stats_interval.tick().await; // discard the immediate first tick

    loop {
        tokio::select! {
            _ = stats_interval.tick() => {
                if require_instance_permission(&state, &user_id, &iid.to_string(), "server:view").await.is_err() {
                    let _ = socket.send(Message::Text(json!({ "type": "error", "message": "console access was revoked" }).to_string())).await;
                    break;
                }
                let stats = collect_stats_for_ws(&state, &iid).await;
                let frame = json!({ "type": "stats", "data": stats });
                if socket.send(Message::Text(frame.to_string())).await.is_err() { break; }
            }
            event = rx.recv() => {
                match event {
                    Ok(Event::InstanceOutput { instance_id, line }) if instance_id == iid => {
                        let frame = json!({ "type": "log", "data": line });
                        if socket.send(Message::Text(frame.to_string())).await.is_err() { break; }
                    }
                    Ok(Event::StatusChanged { instance_id, status }) if instance_id == iid => {
                        let frame = json!({ "type": "state", "data": { "status": status.to_string() } });
                        if socket.send(Message::Text(frame.to_string())).await.is_err() { break; }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(cmd))) => {
                        if require_instance_permission(&state, &user_id, &iid.to_string(), "server:console").await.is_err() {
                            if socket.send(Message::Text(json!({ "type": "error", "message": "console access was revoked" }).to_string())).await.is_err() { break; }
                            continue;
                        }
                        if let Err(error) = send_command(&state, &iid, cmd).await {
                            if socket.send(Message::Text(json!({ "type": "error", "message": error.to_string() }).to_string())).await.is_err() { break; }
                        }
                    }
                    None | Some(Ok(Message::Close(_))) | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

/// Collect CPU/RAM/uptime for a running instance without the player-count probe.
async fn collect_stats_for_ws(
    state: &Arc<AppState>,
    iid: &InstanceId,
) -> Value {
    let (uptime_seconds, pid) = match state.instances.get(iid) {
        Some(h) => (Some(h.started_at.elapsed().as_secs()), h.pid),
        None => {
            return json!({ "cpu_percent": null, "memory_mb": null, "ram_total_mb": null, "player_count": null, "uptime_seconds": null, "total_uptime_seconds": null })
        }
    };

    // Fetch accumulated total from DB (non-blocking; ignore errors)
    let db_total = state
        .instance_store
        .get(iid)
        .await
        .map(|r| r.total_uptime_seconds)
        .unwrap_or(0);
    let total_uptime_seconds = db_total + uptime_seconds.unwrap_or(0);

    let (cpu_percent, memory_mb, ram_total_mb) = if let Some(pid_val) = pid {
        tokio::task::spawn_blocking(move || {
            let p = Pid::from(pid_val as usize);
            let mut sys = System::new_all();
            std::thread::sleep(Duration::from_millis(200));
            sys.refresh_all();
            sys.process(p)
                .map(|proc| {
                    let core_count = sys.cpus().len().max(1) as f32;
                    (
                        Some(proc.cpu_usage() / core_count),
                        Some(proc.memory() / 1_048_576),
                        Some(sys.total_memory() / 1_048_576),
                    )
                })
                .unwrap_or((None, None, None))
        })
        .await
        .unwrap_or((None, None, None))
    } else {
        (None, None, None)
    };

    json!({
        "cpu_percent": cpu_percent,
        "memory_mb": memory_mb,
        "ram_total_mb": ram_total_mb,
        "player_count": null,
        "uptime_seconds": uptime_seconds,
        "total_uptime_seconds": total_uptime_seconds,
    })
}

/// GET /instances/:id/progress — SSE stream for creation progress.
pub async fn sse_progress(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<
    Sse<
        impl futures::Stream<Item = Result<SseEvent, std::convert::Infallible>>,
    >,
    ApiError,
> {
	let iid = resolve_authorized_instance_id(
		&state,
		&claims.sub,
		&id,
		"server:view",
	)
	.await?;

    let stream = BroadcastStream::new(state.broadcaster.subscribe())
        .filter_map(move |msg| {
            let iid = iid.clone();
            async move {
                if let Ok(Event::CreationProgress {
                    instance_id,
                    progress,
                    message,
                }) = msg
                {
                    if instance_id == iid {
                        let data =
                            json!({ "progress": progress, "message": message });
                        return Some(Ok(
                            SseEvent::default().data(data.to_string())
                        ));
                    }
                }
                None
            }
        });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
