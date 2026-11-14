//! `GET /logs`: the end of Core's own log, which only `copal` can read on disk. Owner only.

use std::{path::PathBuf, sync::Arc};

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{CoreLogs, GetLogs},
    server::Req,
};

use super::CoreState;
use crate::{
    access::{Caller, require_owner},
    servers::console,
};

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<GetLogs>,
) -> Result<Json<CoreLogs>, ApiError> {
    require_owner(&caller)?;
    let count = request.query.lines.unwrap_or(100).clamp(1, 1000);
    let lines = match newest(state.servers.paths().logs()) {
        Some(path) => console::tail(&path, count)
            .map(|(lines, _)| lines)
            .unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(Json(CoreLogs { lines }))
}

/// Core's newest log file. Their names end in the date, so the last name is the newest.
fn newest(dir: PathBuf) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("copal."))
        })
        .max()
}
