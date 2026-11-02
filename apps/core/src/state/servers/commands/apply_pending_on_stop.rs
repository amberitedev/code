//! Applying pushed versions as their servers stop, for as long as Core runs.

use std::sync::Arc;

use tokio::sync::Notify;

use super::apply_pending::apply_pending;
use crate::state::servers::{Content, adapters::sqlite::content_rows};

/// Apply each version waiting for its server once the server stops, starting with versions
/// pushed before Core last stopped to servers that are stopped now.
pub async fn apply_pending_on_stop(content: Arc<Content>, owed: Arc<Notify>) {
    use tokio::sync::broadcast::error::RecvError;

    let mut events = content.servers.events.subscribe();
    for id in content_rows::servers_pending(&content.db)
        .await
        .unwrap_or_default()
    {
        tokio::spawn(apply_pending(
            Arc::clone(&content),
            Arc::clone(&owed),
            id,
        ));
    }
    loop {
        match events.recv().await {
            Ok(crate::servers::Lifecycle::Ended(id)) => {
                tokio::spawn(apply_pending(
                    Arc::clone(&content),
                    Arc::clone(&owed),
                    id,
                ));
            }
            Ok(_) | Err(RecvError::Lagged(_)) => {}
            Err(RecvError::Closed) => return,
        }
    }
}
