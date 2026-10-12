//! Servers visible to friends.
//!
//! A server linked to an instance has a "Visible to friends" setting. While it is on and the
//! instance is shared, Core lists the server with the backend, which gives the list to the people
//! the instance is shared with so they can join. Core sends its whole list whenever it differs
//! from the one it last sent, so a change of the setting, a rename, a delete, an unlink, and a new
//! outside address all reach the backend the same way.

use std::{sync::Arc, time::Duration};

use serde::Serialize;
use serde_json::json;
use tracing::warn;

use crate::{
    application::state::{AppState, Owner},
    domain::instance::InstanceId,
};

/// How often the list is compared with the one last sent.
const TICK: Duration = Duration::from_secs(3);

/// A linked server's sharing settings.
#[derive(sqlx::FromRow)]
pub struct Visibility {
    pub visible_to_friends: bool,
    pub shared_instance_id: Option<String>,
}

/// The settings of a linked server. `None` for a server that is not linked.
pub async fn get(
    state: &AppState,
    server_id: &str,
) -> sqlx::Result<Option<Visibility>> {
    sqlx::query_as(
        "SELECT visible_to_friends, shared_instance_id FROM server_sources WHERE instance_id = ?",
    )
    .bind(server_id)
    .fetch_optional(&state.pool)
    .await
}

/// Change a linked server's settings. A part that is `None` is left as it is. Returns false for a
/// server that is not linked.
pub async fn set(
    state: &AppState,
    server_id: &str,
    visible_to_friends: Option<bool>,
    shared_instance_id: Option<Option<String>>,
) -> sqlx::Result<bool> {
    let changed = sqlx::query(
        "UPDATE server_sources SET visible_to_friends = COALESCE(?, visible_to_friends), \
         shared_instance_id = CASE WHEN ? THEN ? ELSE shared_instance_id END WHERE instance_id = ?",
    )
    .bind(visible_to_friends)
    .bind(shared_instance_id.is_some())
    .bind(shared_instance_id.flatten())
    .bind(server_id)
    .execute(&state.pool)
    .await?;
    Ok(changed.rows_affected() > 0)
}

/// One server as the backend lists it.
#[derive(Clone, PartialEq, Serialize)]
struct VisibleServer {
    id: String,
    name: String,
    /// The Minecraft address friends join: the one on the internet while Core knows it,
    /// otherwise the one on Core's own network.
    address: String,
    shared_instance_id: String,
}

async fn visible(state: &AppState) -> sqlx::Result<Vec<VisibleServer>> {
    let rows: Vec<(String, String, u16, String)> = sqlx::query_as(
        "SELECT i.id, i.name, i.port, s.shared_instance_id FROM server_sources s \
         JOIN instances i ON i.id = s.instance_id \
         WHERE s.visible_to_friends = 1 AND s.shared_instance_id IS NOT NULL ORDER BY i.id",
    )
    .fetch_all(&state.pool)
    .await?;
    let host = reqwest::Url::parse(&state.config.public_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| "127.0.0.1".into());
    Ok(rows
        .into_iter()
        .map(|(id, name, port, shared_instance_id)| {
            let outside = id
                .parse::<InstanceId>()
                .ok()
                .and_then(|id| state.network.server(&id))
                .and_then(|port| port.outside);
            VisibleServer {
                address: outside.map_or_else(
                    || format!("{host}:{port}"),
                    |address| address.to_string(),
                ),
                id,
                name,
                shared_instance_id,
            }
        })
        .collect())
}

/// Keeps the backend's list of this Core's visible servers current. Runs for Core's lifetime.
pub async fn run(state: Arc<AppState>) {
    // The key the list was sent with, and the list. A Core signed in again starts over.
    let mut sent: Option<(String, Vec<VisibleServer>)> = None;
    loop {
        tokio::time::sleep(TICK).await;
        let Ok(Some(Owner {
            backend_key: key, ..
        })) = state.owner().await
        else {
            sent = None;
            continue;
        };
        let servers = match visible(&state).await {
            Ok(servers) => servers,
            Err(error) => {
                warn!(%error, "could not read the servers visible to friends");
                continue;
            }
        };
        if sent
            .as_ref()
            .is_some_and(|sent| sent.0 == key && sent.1 == servers)
        {
            continue;
        }
        let response = state
            .http
            .put(format!("{}/core/servers", state.config.account_api_url))
            .bearer_auth(&key)
            .json(&json!({ "servers": servers }))
            .send()
            .await
            .and_then(reqwest::Response::error_for_status);
        match response {
            Ok(_) => sent = Some((key, servers)),
            // Sent again at the next tick.
            Err(error) => {
                warn!(%error, "could not report the servers visible to friends");
            }
        }
    }
}
