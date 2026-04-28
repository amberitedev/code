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
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;

use crate::{
    application::{
        instance_status_service::send_command,
        state::{AppState, WsTicket},
    },
    domain::{event::Event, instance::InstanceId},
    presentation::{error::ApiError, extractors::AuthUser},
};

/// POST /ws-token — issue a 60-second WebSocket ticket (requires auth).
pub async fn issue_ws_token(
    _auth: AuthUser,
    State(state): State<Arc<AppState>>,
) -> Json<Value> {
    let ticket = uuid::Uuid::new_v4().to_string();
    state.ws_tickets.insert(
        ticket.clone(),
        WsTicket { expires_at: Instant::now() + Duration::from_secs(60) },
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
    validate_ticket(&state, &q.ticket)?;

    let iid = id
        .parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id".into()))?;

    if !state.instances.contains_key(&iid) {
        return Err(ApiError::NotFound(format!("instance {id} is not running")));
    }

    let rx = state.broadcaster.subscribe();
    let state_clone = Arc::clone(&state);
    Ok(ws.on_upgrade(move |socket| ws_handler(socket, iid, state_clone, rx)))
}

fn validate_ticket(state: &AppState, ticket: &str) -> Result<(), ApiError> {
    match state.ws_tickets.remove(ticket) {
        Some((_, t)) if t.expires_at > Instant::now() => Ok(()),
        Some(_) => Err(ApiError::Unauthorized("ticket expired".into())),
        None => Err(ApiError::Unauthorized("invalid ticket".into())),
    }
}

async fn ws_handler(
    mut socket: WebSocket,
    iid: InstanceId,
    state: Arc<AppState>,
    mut rx: broadcast::Receiver<Event>,
) {
    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Ok(Event::InstanceOutput { instance_id, line }) if instance_id == iid => {
                        if socket.send(Message::Text(line)).await.is_err() { break; }
                    }
                    Err(_) => break,
                    _ => {}
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(cmd))) => {
                        let _ = send_command(&state, &iid, cmd).await;
                    }
                    None | Some(Ok(Message::Close(_))) | Some(Err(_)) => break,
                    _ => {}
                }
            }
        }
    }
}

/// GET /instances/:id/progress — SSE stream for creation progress.
pub async fn sse_progress(
    _auth: AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Sse<impl futures::Stream<Item = Result<SseEvent, std::convert::Infallible>>>, ApiError>
{
    let iid = id
        .parse::<InstanceId>()
        .map_err(|_| ApiError::BadRequest("invalid instance id".into()))?;

    let stream = BroadcastStream::new(state.broadcaster.subscribe()).filter_map(move |msg| {
        let iid = iid.clone();
        async move {
            if let Ok(Event::CreationProgress { instance_id, progress, message }) = msg {
                if instance_id == iid {
                    let data = json!({ "progress": progress, "message": message });
                    return Some(Ok(SseEvent::default().data(data.to_string())));
                }
            }
            None
        }
    });

    Ok(Sse::new(stream).keep_alive(KeepAlive::default()))
}
