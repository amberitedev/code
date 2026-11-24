//! Serves `ExportDebugInfo` of `crate::local_api::endpoints::ops`.

use crate::state::Desktop;
use std::sync::Arc;

use axum::{Router, extract::State, http::StatusCode};
use contracts::{
    ApiError,
    server::{Mount, Req},
};
use engine::{
    debug_info::{DebugInfoExportStage, export_debug_info},
    routes::api_error,
};

use crate::local_api::{
    AppState,
    endpoints::ops::ExportDebugInfo,
    ops::{Ops, cancelled},
};

pub fn routes() -> Router<AppState> {
    Router::new().endpoint::<ExportDebugInfo, _, _>(export)
}

async fn export(
    State(engine): State<Desktop>,
    State(ops): State<Arc<Ops>>,
    Req { body, .. }: Req<ExportDebugInfo>,
) -> Result<StatusCode, ApiError> {
    ops.start(body.op, |op| async move {
        let cancel = op.cancel_token();
        let saved =
            export_debug_info(&engine, body.path, cancel, move |progress| {
                let stage = match progress.stage {
                    DebugInfoExportStage::Preparing => "preparing",
                    DebugInfoExportStage::Exporting => "exporting",
                    DebugInfoExportStage::Finishing => "finishing",
                };
                op.progress(
                    Some(stage),
                    progress.processed_bytes,
                    progress.total_bytes,
                );
            })
            .await
            .map_err(api_error)?;
        if saved { Ok(None) } else { Err(cancelled()) }
    })
}
