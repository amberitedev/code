//! A server letting go of its own share (`api::share`): when it is deleted, unshared, or starts
//! following an instance, since a server never has two shares. The backend is owed the share's
//! deletion, so its players stop getting it; `access::sync` makes the call and treats a share
//! already gone, or no longer this Core's, as deleted.

use std::sync::Arc;

use sqlx::{SqliteConnection, SqlitePool};
use tokio::sync::Notify;

use crate::{
    access::owed::OwedCall, state::servers::adapters::sqlite::server_rows,
};

/// Let go of the server's own share, if it has one, in the caller's transaction: the server
/// stops being shared now, and the backend is owed the share's deletion. Wake sync with
/// `Access::owed` once it commits. Returns whether there was a share.
pub async fn owe_share_deletion(
    conn: &mut SqliteConnection,
    id: &str,
) -> sqlx::Result<bool> {
    let Some(share_id) = server_rows::share_id(&mut *conn, id).await? else {
        return Ok(false);
    };
    server_rows::set_share_id(&mut *conn, id, None).await?;
    crate::access::sync::owe(conn, &OwedCall::DeleteShare { share_id }).await?;
    Ok(true)
}

/// Let go of the server's own share now, and wake sync (`owed`) to tell the backend. Returns
/// whether there was a share.
pub async fn drop_own_share(
    db: &SqlitePool,
    owed: &Arc<Notify>,
    id: &str,
) -> sqlx::Result<bool> {
    let mut tx = db.begin().await?;
    let dropped = owe_share_deletion(&mut tx, id).await?;
    tx.commit().await?;
    if dropped {
        owed.notify_one();
    }
    Ok(dropped)
}
