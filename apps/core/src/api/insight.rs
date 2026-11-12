//! What Core knows about a server beyond its power state, and about the machine.

use std::sync::Arc;

use axum::extract::State;
use contracts::{
    ApiError, Json,
    core::{
        CheckReachability, Crash, GetInsight, GetMachine, Machine, Reach,
        ReachabilityCheck, ReachabilityReport, ServerInsight,
    },
    server::Req,
};

use super::{
    CoreState, internal,
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
pub fn archon(state: &CoreState, server_id: &str) -> Option<Reachability> {
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
    state: &CoreState,
    id: &str,
    level: Level,
) -> Result<ServerInsight, ApiError> {
    let last_crash: Option<(i64, Option<i32>, Option<String>)> = sqlx::query_as(
        "SELECT ended_at, exit_code, last_lines FROM runs \
         WHERE server_id = ? AND outcome = 'crashed' ORDER BY ended_at DESC LIMIT 1",
    )
    .bind(id)
    .fetch_optional(&state.db)
    .await.map_err(internal)?;
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

pub async fn get(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<GetInsight>,
) -> Result<Json<ServerInsight>, ApiError> {
    let id = request.params.id;
    let level = caller.require(&state.db, &id, Level::Viewer).await?;
    Ok(Json(insight(&state, &id, level).await?))
}

pub async fn check(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<CheckReachability>,
) -> Result<Json<ServerInsight>, ApiError> {
    let id = request.params.id;
    let level = caller.require(&state.db, &id, Level::Editor).await?;
    if state.servers.phase(&id).is_none() {
        return Err(ServerError::NotRunning.into());
    }
    Arc::clone(&state.network)
        .check(Arc::clone(&state.servers), id.clone())
        .await;
    Ok(Json(insight(&state, &id, level).await?))
}

pub async fn machine(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetMachine>,
) -> Result<Json<Machine>, ApiError> {
    caller.require_owner()?;
    let memory_mb = memory::machine_mb();
    let assigned_mb = memory::assigned_mb(&state.db).await.map_err(internal)?;
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
