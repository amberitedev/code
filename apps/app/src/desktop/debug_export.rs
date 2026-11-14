//! Serves `ExportDebugInfo` of `crate::server::endpoints::ops`.

use std::sync::Arc;

use axum::{Router, extract::State, http::StatusCode};
use contracts::{
    ApiError,
    server::{Req, route},
};
use engine::{
    Engine,
    debug_info::{DebugInfoExportStage, export_debug_info},
    routes::api_error,
};

use crate::server::{
    AppState,
    endpoints::ops::ExportDebugInfo,
    ops::{Ops, cancelled},
};

pub fn routes() -> Router<AppState> {
    let (path, export) = route::<ExportDebugInfo, _, _, _>(export);
    Router::new().route(path, export)
}

async fn export(
    State(engine): State<Arc<Engine>>,
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
