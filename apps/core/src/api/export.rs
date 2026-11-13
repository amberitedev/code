//! Exporting a server's content as a Modrinth `.mrpack` (`state::servers::commands::export_mrpack`).
//! The App saves it natively, so it asks for a link first (`access::links`), as for a world
//! download.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Response,
};
use contracts::ApiError;
use uuid::Uuid;

use super::{CoreState, backups::zip_response, content::world, internal};
use crate::{
    access::{Caller, Permission},
    servers::ServerError,
    state::servers::{
        adapters::sqlite::server_rows, commands::export_mrpack::export_mrpack,
    },
};

/// `GET /servers/{id}/v1/worlds/{wid}/content/export-mrpack`: the server's content as an
/// `.mrpack`, written to its staging folder, streamed, then removed.
pub async fn export(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    world(&id, &wid)?;
    state.access.require(&caller, &id, Permission::Edit).await?;
    let server = server_rows::get(&state.db, &id)
        .await
        .map_err(internal)?
        .ok_or_else(|| ServerError::NotFound(id.clone()))?;
    let path = state
        .content
        .paths
        .staging(&id)
        .join(format!("export-{}.mrpack", Uuid::new_v4()));
    if let Err(error) = export_mrpack(&state.content, &server, &path).await {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(error.into());
    }
    let file = std::fs::File::open(&path).map_err(internal)?;
    Ok(zip_response(file, &server.name, "mrpack", Some(path)))
}
