//! Whether a server-first server has changes its last push does not have.

use sharing_client::{diff::has_changes, history};

use super::share_snapshot::{
    history_store, push_options, share_contents, snapshot,
};
use crate::state::servers::{Content, Error, adapters::sqlite::server_rows};

/// Whether the server has changes its last push does not have, so a push would change something
/// (upstream's `shared_instance_needs_update`). Compared with the newest entry of the share's
/// history on this Core, so it asks no backend. False for a server that is not shared, and when
/// this Core has no push of it.
pub async fn needs_update(content: &Content, id: &str) -> bool {
    let check = async {
        let Some(share_id) = server_rows::share_id(&content.db, id).await?
        else {
            return Ok(false);
        };
        let Some(last) =
            history::latest(&history_store(content), &share_id).await?
        else {
            return Ok(false);
        };
        let snapshot = snapshot(content, id).await?;
        let files = last.manifest().cloned().collect::<Vec<_>>();
        let (before, after) =
            share_contents(&snapshot, &last.content, &files).await?;
        Ok::<_, Error>(has_changes(
            &before,
            &after,
            push_options().common_external_files,
        ))
    };
    check.await.unwrap_or_else(|error| {
        tracing::debug!(server = %id, %error, "could not compare the server to its last push");
        false
    })
}
