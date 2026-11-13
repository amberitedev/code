//! Serves `UploadLocalFile` of `crate::server::endpoints::ops`: large files go from disk to the
//! server without passing through the page.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use axum::{Router, extract::State, http::StatusCode};
use contracts::{
    ApiError, ErrorCode,
    server::{Req, route},
};
use engine::{
    routes::api_error,
    upload::{LocalFileUpload, upload_local_file},
};

use crate::server::{
    AppState,
    endpoints::ops::{OpResponse, UploadLocalFile},
    ops::Ops,
};

pub fn routes() -> Router<AppState> {
    let (path, upload) = route::<UploadLocalFile, _, _, _>(upload);
    Router::new().route(path, upload)
}

async fn upload(
    State(ops): State<Arc<Ops>>,
    Req { body, .. }: Req<UploadLocalFile>,
) -> Result<StatusCode, ApiError> {
    let url = url::Url::parse(&body.url).map_err(|error| {
        ApiError::new(ErrorCode::InvalidInput, error.to_string())
    })?;
    ops.start(body.op, |op| async move {
        let reporter = op.clone();
        let mut last_progress: Option<Instant> = None;
        let upload = upload_local_file(
            LocalFileUpload {
                url,
                headers: body.headers.into_iter().collect(),
                path: body.path,
                file_name: body.file_name,
                multipart_field: body.multipart_field,
            },
            move |loaded, total| {
                if loaded == total
                    || last_progress.is_none_or(|last| {
                        last.elapsed() >= Duration::from_millis(100)
                    })
                {
                    reporter.progress(None, loaded, Some(total));
                    last_progress = Some(Instant::now());
                }
            },
        );
        let response = op
            .until_cancelled(async { upload.await.map_err(api_error) })
            .await?;
        Ok(Some(OpResponse {
            status: response.status,
            body: response.body,
        }))
    })
}
