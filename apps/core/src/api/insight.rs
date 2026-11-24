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

use super::{CoreState, internal};
use crate::{
    access::{Caller, require_owner},
    network::{
        diagnosis::{Case, Mark},
        forward::{self, Report},
    },
    servers::{ServerError, memory, server_rows},
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

async fn insight(
    state: &CoreState,
    id: &str,
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
            last_lines: last_lines.unwrap_or_default(),
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
    state.access.require(&caller, &id).await?;
    Ok(Json(insight(&state, &id).await?))
}

pub async fn check(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    request: Req<CheckReachability>,
) -> Result<Json<ServerInsight>, ApiError> {
    let id = request.params.id;
    state.access.require(&caller, &id).await?;
    if state.servers.phase(&id).is_none() {
        return Err(ServerError::NotRunning.into());
    }
    Arc::clone(&state.network)
        .check(Arc::clone(&state.servers), id.clone())
        .await;
    Ok(Json(insight(&state, &id).await?))
}

pub async fn machine(
    caller: Caller,
    State(state): State<Arc<CoreState>>,
    _: Req<GetMachine>,
) -> Result<Json<Machine>, ApiError> {
    require_owner(&caller)?;
    let memory_mb = memory::machine_mb().await;
    let assigned_mb = server_rows::list(&state.db, &state.engine)
        .await
        .map_err(internal)?
        .iter()
        .map(|server| server.memory_mb)
        .sum::<i64>();
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
