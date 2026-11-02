//! Applying a share version that waited for its server to stop (see `install_instance`).

use std::sync::Arc;

use tokio::sync::Notify;

use super::install_pack::{self as content_install, PackSource};
use crate::state::servers::{
    Content, Installing, SharedInstanceLink, adapters::sqlite::content_rows,
};

/// Apply the version waiting for a server, when one waits and the server is stopped. Starts the
/// server again afterwards when the push asked for a restart. Another change to the server's
/// content is waited for, so it never costs the version. `owed` wakes sync, since another
/// instance may have dropped the players of the one the server followed.
pub async fn apply_pending(
    content: Arc<Content>,
    owed: Arc<Notify>,
    id: String,
) {
    let id = id.as_str();
    let busy = content.busy_wait(id).await;
    let Ok(Some(pending)) = content_rows::get_pending(&content.db, id).await
    else {
        return;
    };
    let Ok(hold) = content.servers.hold(id) else {
        return;
    };
    if content_rows::clear_pending(&content.db, id).await.is_err() {
        return;
    }
    let (start_after, force) = (pending.start_after, pending.force);
    let source = PackSource::Stored {
        sha1: pending.sha1,
        instance: Some(SharedInstanceLink {
            instance_id: pending.instance_id,
            instance_name: pending.instance_name,
            shared_instance_id: pending.shared_instance_id,
            version: pending.shared_version,
        }),
    };
    content.spawn(
        hold,
        busy,
        Installing::Modpack,
        move |content, hold, progress| async move {
            let result = content_install::pack(
                &content, &hold, &progress, source, None, force,
            )
            .await;
            owed.notify_one();
            if start_after {
                // Back up either way: a failed update changes nothing it could not finish.
                let id = hold.id().to_owned();
                drop(hold);
                if let Err(error) = content.servers.start(&id).await {
                    tracing::warn!(server = %id, %error, "could not start the server after its update");
                }
            }
            result
        },
    );
}
