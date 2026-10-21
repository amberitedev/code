//! `GET /logs`: the end of Core's own log, which only `copal` can read on disk. Owner only.
//! [`LogsQuery`] as query parameters, [`CoreLogs`] out.

use std::{path::PathBuf, sync::Arc};

use axum::{
    Json,
    extract::{Query, State},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{AppState, error::ApiError};
use crate::{access::Caller, servers::console};

#[derive(Debug, Clone, Deserialize, TS)]
pub struct LogsQuery {
    /// At most 1000. 100 when left out.
    #[ts(optional)]
    pub lines: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct CoreLogs {
    /// Oldest first.
    pub lines: Vec<String>,
}

/// `GET /logs`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<CoreLogs>, ApiError> {
    caller.require_owner()?;
    let count = query.lines.unwrap_or(100).clamp(1, 1000);
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
