//! A push of a server-first server, kept in the share's history on this Core.

use sharing_client::{
    diff::{ShareContent, share_diff},
    history, push,
};

use super::share_snapshot::{
    Names, Snapshot, configuration, history_store, push_options,
    version_content,
};
use crate::state::servers::{Content, Error};

/// Records `pushed` in the share's history, naming what it changed over the version before it.
/// The push already succeeded, so a failure is logged and the version is only missing from the
/// history.
pub async fn record_push(
    content: &Content,
    snapshot: &Snapshot,
    share_id: &str,
    pushed: &push::Pushed,
) {
    let diffs = async {
        let before = match &pushed.previous {
            Some(previous) => version_content(
                snapshot,
                &previous.content,
                &previous.external_files,
            )?,
            None => ShareContent::empty(configuration(&snapshot.content)),
        };
        let after = version_content(
            snapshot,
            &snapshot.content,
            &pushed.external_files,
        )?;
        share_diff(&before, &after, &push_options(), &Names(content))
            .await
            .map_err(|error| Error::Other(color_eyre::eyre::eyre!("{error}")))
    };
    let diffs = diffs.await.unwrap_or_else(|error| {
        tracing::warn!(share = %share_id, %error, "could not name what a push changed");
        Vec::new()
    });
    if let Err(error) = history::record(
        &history_store(content),
        share_id,
        &snapshot.content,
        pushed,
        diffs,
        Some(history::DEFAULT_KEPT),
    )
    .await
    {
        tracing::warn!(share = %share_id, %error, "could not record a push in the share's history");
    }
}
