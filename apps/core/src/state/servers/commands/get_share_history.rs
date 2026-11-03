//! The history of a server-first server's share, as its History page lists it (app-lib's
//! `get_shared_instance_history`).

use sharing_client::history::{self, SharedInstanceHistoryEntry};

use super::share_snapshot::history_store;
use crate::state::servers::{Content, Error, adapters::sqlite::server_rows};

/// The versions of the server's share this Core pushed, newest first. Empty while it is not
/// shared.
pub async fn get_share_history(
    content: &Content,
    id: &str,
) -> Result<Vec<SharedInstanceHistoryEntry>, Error> {
    let Some(share_id) = server_rows::share_id(&content.db, id).await? else {
        return Ok(Vec::new());
    };
    Ok(history::list(&history_store(content), &share_id)
        .await?
        .into_iter()
        .map(SharedInstanceHistoryEntry::from)
        .collect())
}
