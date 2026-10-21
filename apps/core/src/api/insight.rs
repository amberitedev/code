//! What Core knows about a server beyond its power state, and about the machine.
//!
//! - `GET /servers/{id}/insight`: [`ServerInsight`]. Viewers and up; the console lines of a
//!   crash are for editors and up.
//! - `POST /servers/{id}/reachability`: check the server's port again, answering once checked.
//!   [`ServerInsight`]. Editors and up.
//! - `GET /machine`: [`Machine`]. Owner only.

use std::sync::Arc;

use axum::{
    Json,
    extract::{Path, State},
};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    AppState,
    error::ApiError,
    servers::{Reachability, ReachabilityState},
};
use crate::{
    access::{Caller, Level},
    network::{
        diagnosis::{Case, Mark},
        forward::{self, Report},
    },
    servers::{ServerError, memory},
};

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ServerInsight {
    /// Who is online, from join and leave lines. Empty while the server does not run.
    pub players: Vec<String>,
    /// `list` answered this run, so `players` is checked, not only what join and leave lines
    /// said.
    pub players_checked: bool,
    /// The server stopped answering Core's probe, so it is probably frozen. Offer a restart.
    pub frozen: bool,
    /// Unix seconds of the server's last answer to the probe.
    pub answered_at: Option<i64>,
    /// Core stopped starting the server again after it crashed 3 times in 10 minutes.
    pub crash_restart_stopped: bool,
    /// The last crash, with the console lines before it for editors and up.
    pub last_crash: Option<Crash>,
    /// Whether players can join from the internet. Null while the server does not run or has
    /// not been checked yet.
    pub reachability: Option<ReachabilityReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Crash {
    /// Unix seconds.
    pub at: i64,
    pub exit_code: Option<i32>,
    /// The console lines before the crash. Null for viewers: they can hold chat, addresses,
    /// and anything a mod printed, which the console route keeps to editors.
    pub last_lines: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReachabilityReport {
    pub state: Reach,
    /// The cause in one line.
    pub title: String,
    /// The one thing to do next, when there is something.
    pub next_step: Option<String>,
    /// Each test, in order.
    pub checks: Vec<ReachabilityCheck>,
    pub found: Vec<String>,
    /// Every step of the fix, `next_step` first.
    pub fix: Vec<String>,
    /// Unix seconds.
    pub checked_at: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
pub enum Reach {
    Reachable,
    /// Forwarded, but not tested from the internet.
    Unverified,
    Unreachable,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct ReachabilityCheck {
    /// Null when the test was not run or could not tell.
    pub passed: Option<bool>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, TS)]
pub struct Machine {
    /// This machine's memory. Null when the OS does not say.
    pub memory_mb: Option<u64>,
    /// The memory every server together may use.
    pub assigned_mb: i64,
    /// Set when the servers together may use more than the machine has.
    pub warning: Option<String>,
    /// Whether the App can reach Core's API from the internet.
    pub api_reachability: Option<ReachabilityReport>,
}

impl From<Report> for ReachabilityReport {
    fn from(report: Report) -> Self {
        let diagnosis = report.diagnosis;
        Self {
            state: match diagnosis.case {
                Case::Reachable => Reach::Reachable,
                Case::Unverified => Reach::Unverified,
                _ => Reach::Unreachable,
            },
            title: diagnosis.title,
            next_step: diagnosis.fix.first().cloned(),
            checks: diagnosis
                .checks
                .into_iter()
                .map(|check| ReachabilityCheck {
                    passed: match check.mark {
                        Mark::Pass => Some(true),
                        Mark::Fail => Some(false),
                        Mark::Skip => None,
                    },
                    text: check.text,
                })
                .collect(),
            found: diagnosis.found,
            fix: diagnosis.fix,
            checked_at: report.at,
        }
    }
}

/// The hosting API's short form of a server's reachability.
pub fn archon(state: &AppState, server_id: &str) -> Option<Reachability> {
    let report = ReachabilityReport::from(state.network.report(server_id)?);
    Some(Reachability {
        state: match report.state {
            Reach::Reachable => ReachabilityState::Reachable,
            Reach::Unverified => ReachabilityState::Unverified,
            Reach::Unreachable => ReachabilityState::Unreachable,
        },
        cause: (report.state != Reach::Reachable).then_some(report.title),
    })
}

/// What `level` may see of a server.
async fn insight(
    state: &AppState,
    id: &str,
    level: Level,
) -> Result<ServerInsight, ApiError> {
    let last_crash: Option<(i64, Option<i32>, Option<String>)> = sqlx::query_as(
        "SELECT ended_at, exit_code, last_lines FROM runs \
         WHERE server_id = ? AND outcome = 'crashed' ORDER BY ended_at DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await?;
    let live = state.servers.insight(id).unwrap_or_default();
    Ok(ServerInsight {
        players: live.players.into_iter().collect(),
        players_checked: live.players_checked,
        frozen: live.frozen,
        answered_at: live.answered_at,
        crash_restart_stopped: state.servers.gave_up(id),
        last_crash: last_crash.map(|(at, exit_code, last_lines)| Crash {
            at,
            exit_code,
            last_lines: (level >= Level::Editor)
                .then(|| last_lines.unwrap_or_default()),
        }),
        reachability: state.network.report(id).map(Into::into),
    })
}

/// `GET /servers/{id}/insight`.
pub async fn get(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ServerInsight>, ApiError> {
    let level = caller.require(&state.db, &id, Level::Viewer).await?;
    Ok(Json(insight(&state, &id, level).await?))
}

/// `POST /servers/{id}/reachability`.
pub async fn check(
    caller: Caller,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<Json<ServerInsight>, ApiError> {
    let level = caller.require(&state.db, &id, Level::Editor).await?;
    if state.servers.phase(&id).is_none() {
        return Err(ServerError::NotRunning.into());
    }
    Arc::clone(&state.network)
        .check(Arc::clone(&state.servers), id.clone())
        .await;
    Ok(Json(insight(&state, &id, level).await?))
}

/// `GET /machine`.
pub async fn machine(
    caller: Caller,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Machine>, ApiError> {
    caller.require_owner()?;
    let memory_mb = memory::machine_mb();
    let assigned_mb = memory::assigned_mb(&state.db).await?;
    let warning = memory_mb
        .filter(|&machine| assigned_mb > machine as i64)
        .map(|machine| {
            format!(
                "The servers may use {assigned_mb} MB together, more than this machine's {machine} MB. Running them all at once can slow the machine down or stop servers."
            )
        });
    Ok(Json(Machine {
        memory_mb,
        assigned_mb,
        warning,
        api_reachability: state.network.report(forward::CORE).map(Into::into),
    }))
}
