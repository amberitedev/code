//! A server letting go of its own share (`api::share`): when it is deleted, unshared, or starts
//! following an instance, since a server never has two shares. The backend is owed the share's
//! deletion, so its players stop getting it; `access::sync` makes the call and treats a share
//! already gone, or no longer this Core's, as deleted. The share's local history goes with it.

use std::sync::Arc;

use sqlx::SqliteConnection;
use tokio::sync::Notify;

use crate::{
    access::owed::OwedCall,
    state::servers::{
        Content,
        adapters::{history::History, sqlite::server_rows},
    },
};

/// Let go of the server's own share, if it has one, in the caller's transaction: the server
/// stops being shared now, and the backend is owed the share's deletion. Wake sync with
/// `Access::owed` and [`release_history`] once it commits. Returns the share, if there was one.
pub async fn owe_share_deletion(
    conn: &mut SqliteConnection,
    id: &str,
) -> sqlx::Result<Option<String>> {
    let Some(share_id) = server_rows::share_id(&mut *conn, id).await? else {
        return Ok(None);
    };
    server_rows::set_share_id(&mut *conn, id, None).await?;
    crate::access::sync::owe(
        conn,
        &OwedCall::DeleteShare {
            share_id: share_id.clone(),
        },
    )
    .await?;
    Ok(Some(share_id))
}

/// Let go of the server's own share now, wake sync (`owed`) to tell the backend, and release
/// the share's history. Returns whether there was a share.
pub async fn drop_own_share(
    content: &Content,
    owed: &Arc<Notify>,
    id: &str,
) -> sqlx::Result<bool> {
    let mut tx = content.db.begin().await?;
    let dropped = owe_share_deletion(&mut tx, id).await?;
    tx.commit().await?;
    let Some(share_id) = dropped else {
        return Ok(false);
    };
    owed.notify_one();
    release_history(content, &share_id).await;
    Ok(true)
}

/// Deletes the local history of `share_id`, a share the server let go of, and lets the content
/// store clean up its files. The share is already gone, so a failure is only logged.
pub async fn release_history(content: &Content, share_id: &str) {
    let history = History {
        store: &content.content_store,
        db: &content.db,
    };
    if let Err(error) =
        sharing_client::history::prune(&history, share_id, Some(0)).await
    {
        tracing::warn!(share_id, %error, "could not release a share's history");
    }
}
