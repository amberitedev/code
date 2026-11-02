//! Bringing what Core records in line with what is known about the files on disk.

use tracing::warn;

use crate::state::servers::{
    Content, TrackedFile, adapters::sqlite::content_rows,
};

/// Fill in the Modrinth project of tracked files that lack one, by their hash. Best effort.
pub async fn identify(content: &Content, server_id: &str) {
    let Ok(rows) =
        content_rows::get_tracked_files(&content.db, server_id).await
    else {
        return;
    };
    let unknown: Vec<&TrackedFile> =
        rows.iter().filter(|row| row.project_id.is_none()).collect();
    if unknown.is_empty() {
        return;
    }
    let sha1s: Vec<String> =
        unknown.iter().map(|row| row.sha1.clone()).collect();
    match content.modrinth.by_hashes(&sha1s).await {
        Ok(found) => {
            for row in unknown {
                if let Some(version) = found.get(&row.sha1) {
                    let _ = content_rows::set_project(
                        &content.db,
                        server_id,
                        &row.relative_path,
                        &version.project_id,
                        &version.id,
                    )
                    .await;
                }
            }
        }
        Err(error) => {
            warn!(server = %server_id, %error, "could not look files up on Modrinth")
        }
    }
}
