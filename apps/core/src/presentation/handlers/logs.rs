use std::sync::Arc;

use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, HeaderValue, StatusCode},
    response::Response,
    Json,
};
use serde_json::{json, Value};

use crate::{
    application::{
        log_service::{
            list_crash_reports, list_logs, resolve_crash, resolve_log,
        },
        state::AppState,
    },
    presentation::{
        error::ApiError, extractors::AuthUser,
        instance_path::resolve_authorized_instance_id,
    },
};

/// GET /instances/:id/logs
pub async fn list_logs_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_authorized_instance_id(&state, &claims.sub, &id, "server:logs")
            .await?
            .to_string();
    let logs = list_logs(&state, &instance_id).await?;
    Ok(Json(json!({ "logs": logs })))
}

/// GET /instances/:id/logs/:filename
pub async fn read_log_handler(
    AuthUser(claims): AuthUser,
    Path((id, filename)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let instance_id =
        resolve_authorized_instance_id(&state, &claims.sub, &id, "server:logs")
            .await?
            .to_string();
    let (path, is_gzipped) =
        resolve_log(&state, &instance_id, &filename).await?;
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;

    let mut builder = Response::builder().status(StatusCode::OK);
    if is_gzipped {
        builder = builder
            .header(header::CONTENT_ENCODING, HeaderValue::from_static("gzip"))
            .header(
                header::CONTENT_TYPE,
                HeaderValue::from_static("text/plain"),
            );
    } else {
        builder = builder.header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        );
    }
    builder
        .body(Body::from(data))
        .map_err(|e| ApiError::Internal(e.to_string()))
}

/// GET /instances/:id/crash-reports
pub async fn list_crash_reports_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let instance_id =
        resolve_authorized_instance_id(&state, &claims.sub, &id, "server:logs")
            .await?
            .to_string();
    let reports = list_crash_reports(&state, &instance_id).await?;
    Ok(Json(json!({ "crash_reports": reports })))
}

/// GET /instances/:id/crash-reports/:filename
pub async fn read_crash_report_handler(
    AuthUser(claims): AuthUser,
    Path((id, filename)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
    let instance_id =
        resolve_authorized_instance_id(&state, &claims.sub, &id, "server:logs")
            .await?
            .to_string();
    let path = resolve_crash(&state, &instance_id, &filename).await?;
    let data = tokio::fs::read(&path)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_TYPE,
            HeaderValue::from_static("text/plain; charset=utf-8"),
        )
        .body(Body::from(data))
        .map_err(|e| ApiError::Internal(e.to_string()))
}
