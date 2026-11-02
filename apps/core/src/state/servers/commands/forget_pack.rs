//! Deleting a stored pack nothing uses any more, such as a waiting version a newer push
//! replaced.

use crate::{
    installs::shared::SharedDirs,
    state::servers::{Content, adapters::sqlite::content_rows},
};

/// Delete the stored pack `sha1` unless a server still uses it.
pub async fn forget_pack(content: &Content, sha1: Option<String>) {
    let Some(sha1) = sha1 else { return };
    if let Ok(false) = content_rows::pack_in_use(&content.db, &sha1).await {
        let dirs = SharedDirs::new(content.servers.paths().root());
        let _ = tokio::fs::remove_file(dirs.pack(&sha1)).await;
    }
}
