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
    let lines = tail(state.servers.paths().logs(), count);
    Ok(Json(CoreLogs { lines }))
}

/// The last `count` lines of Core's newest log in `dir`, oldest first. Empty when there is no
/// log or this account may not read it. The CLI reads it this way when Core does not answer.
pub fn tail(dir: PathBuf, count: usize) -> Vec<String> {
    newest(dir)
        .and_then(|path| console::tail(&path, count).ok())
        .map(|(lines, _)| lines)
        .unwrap_or_default()
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
