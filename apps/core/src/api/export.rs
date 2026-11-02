//! Exporting a server's content as a Modrinth `.mrpack` (`state::servers::commands::export_mrpack`).
//! The App saves it natively, so it asks for a link token first, as for a world download.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, Query, State},
    response::Response,
};
use uuid::Uuid;

use super::{
    AppState,
    backups::{DownloadQuery, DownloadToken, downloader, zip_response},
    content::world,
    error::ApiError,
};
use crate::{
    access::{Caller, Level},
    backups::Target,
    servers::ServerError,
    state::servers::{
        adapters::sqlite::server_rows, commands::export_mrpack::export_mrpack,
    },
};

/// `GET /servers/{id}/v1/worlds/{wid}/content/export-mrpack/authorize`: a link token for one
/// export.
pub async fn authorize(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> Result<Json<DownloadToken>, ApiError> {
    caller.require(&state.db, &id, Level::Editor).await?;
    world(&id, &wid)?;
    let token = state.backups.grant(caller, &id, Target::Pack);
    Ok(Json(DownloadToken { token }))
}

/// `GET /servers/{id}/v1/worlds/{wid}/content/export-mrpack`: the server's content as an
/// `.mrpack`, written to its staging folder, streamed, then removed. The link token goes in
/// `token`; a session in the header works too.
pub async fn export(
    caller: Result<Caller, ApiError>,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Query(query): Query<DownloadQuery>,
) -> Result<Response, ApiError> {
    world(&id, &wid)?;
    downloader(&state, caller, query.token.as_deref(), &id, &Target::Pack)
        .await?;
    let server = server_rows::get(&state.db, &id)
        .await?
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
    let file = std::fs::File::open(&path)?;
    Ok(zip_response(file, &server.name, "mrpack", Some(path)))
}
