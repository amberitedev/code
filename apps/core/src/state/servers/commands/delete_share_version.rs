//! Deleting a version from a server-first server's share history (app-lib's
//! `delete_shared_instance_version`).

use sharing_client::history;

use super::share_snapshot::history_store;
use crate::state::servers::{Content, Error, adapters::sqlite::server_rows};

/// Deletes `version` from the server's share history. The newest entry is the last push, so it
/// stays (`sharing_client::history::delete_older`).
pub async fn delete_share_version(
    content: &Content,
    id: &str,
    version: i32,
) -> Result<(), Error> {
    let share_id = server_rows::share_id(&content.db, id)
        .await?
        .ok_or_else(|| Error::NotFound("This server is not shared.".into()))?;
    Ok(
        history::delete_older(&history_store(content), &share_id, version)
            .await?,
    )
}
