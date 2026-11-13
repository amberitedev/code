//! Exporting a server's content as a Modrinth `.mrpack`, through its engine instance. The App saves
//! it natively, so it asks for a link first (`access::links`), as for a world download. A server
//! exports what upstream's export selects by default: its content folders and `config/`.

use std::sync::Arc;

use axum::{
    extract::{Path, State},
    response::Response,
};
use contracts::ApiError;
use uuid::Uuid;

use super::{CoreState, backups::zip_response, internal, world};
use crate::{
    access::{Caller, Permission},
    servers::{ServerError, server_rows},
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
    let staging = state.servers.paths().staging(&id);
    tokio::fs::create_dir_all(&staging)
        .await
        .map_err(internal)?;
    let path = staging.join(format!("export-{}.mrpack", Uuid::new_v4()));
    let level =
        engine::sandbox::Folder::open(&state.servers.paths().server(&id))
            .map_err(internal)?
            .level_name();
    let included = ["mods", "plugins", "config"]
        .map(str::to_owned)
        .into_iter()
        .chain([format!("{level}/datapacks")])
        .collect();
    let exported = engine::instance::export_mrpack(
        &state.engine,
        &id,
        path.clone(),
        included,
        Vec::new(),
        None,
        None,
        None,
    )
    .await;
    if let Err(error) = exported {
        let _ = tokio::fs::remove_file(&path).await;
        return Err(engine::routes::api_error(error));
    }
    let file = std::fs::File::open(&path).map_err(internal)?;
    Ok(zip_response(file, &server.name, "mrpack", Some(path)))
}
