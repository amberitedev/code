use std::sync::Arc;

use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{
        header::{HeaderName, HeaderValue, LOCATION},
        HeaderMap, StatusCode,
    },
    response::{IntoResponse, Response},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::Deserialize;

use crate::{
    application::{
        state::AppState,
        upload_service::{
            append_upload, cancel_upload, create_upload_session, upload_status,
            UploadError,
        },
	},
	presentation::{
		error::ApiError, extractors::AuthUser,
		instance_path::resolve_authorized_instance_id,
	},
};

const UPLOAD_LENGTH: HeaderName = HeaderName::from_static("upload-length");
const UPLOAD_OFFSET: HeaderName = HeaderName::from_static("upload-offset");
const UPLOAD_METADATA: HeaderName = HeaderName::from_static("upload-metadata");
const UPLOAD_CHECKSUM: HeaderName = HeaderName::from_static("upload-checksum");

#[derive(Debug, Deserialize)]
pub struct CreateUploadQuery {
    pub path: String,
}

pub async fn create_upload_handler(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Query(query): Query<CreateUploadQuery>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
	let instance_id = resolve_upload_instance_id(&state, &claims.sub, &id)
		.await?;
	let length = parse_required_u64(&headers, &UPLOAD_LENGTH)?;
	let sha256 = upload_metadata(&headers, "sha256")?;
	let session =
		create_upload_session(&state, &instance_id, &query.path, length, sha256)
			.await
			.map_err(upload_error)?;
	let location = format!(
		"/instances/{}/fs/uploads/{}",
		encode_path_segment(&id),
		session.id
	);
	let mut response = StatusCode::CREATED.into_response();
    let headers = response.headers_mut();
    headers.insert(LOCATION, header_value(&location)?);
    headers.insert(UPLOAD_OFFSET, HeaderValue::from_static("0"));
    headers.insert(UPLOAD_LENGTH, header_value(&session.length.to_string())?);
    Ok(response)
}

pub async fn upload_status_handler(
    AuthUser(claims): AuthUser,
    Path((id, upload_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
	let instance_id = resolve_upload_instance_id(&state, &claims.sub, &id)
		.await?;
	let session =
		upload_status(&state, &instance_id, &upload_id)
			.map_err(upload_error)?;
	let mut response = StatusCode::NO_CONTENT.into_response();
    let headers = response.headers_mut();
    headers.insert(UPLOAD_OFFSET, header_value(&session.offset.to_string())?);
    headers.insert(UPLOAD_LENGTH, header_value(&session.length.to_string())?);
    Ok(response)
}

pub async fn append_upload_handler(
    AuthUser(claims): AuthUser,
    Path((id, upload_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    body: Bytes,
) -> Result<Response, ApiError> {
	let instance_id = resolve_upload_instance_id(&state, &claims.sub, &id)
		.await?;
	let offset = parse_required_u64(&headers, &UPLOAD_OFFSET)?;
	let checksum = upload_checksum(&headers)?;
	let session =
		append_upload(&state, &instance_id, &upload_id, offset, body, checksum)
			.await
			.map_err(upload_error)?;
    let mut response = StatusCode::NO_CONTENT.into_response();
    response
        .headers_mut()
        .insert(UPLOAD_OFFSET, header_value(&session.offset.to_string())?);
    Ok(response)
}

pub async fn cancel_upload_handler(
    AuthUser(claims): AuthUser,
    Path((id, upload_id)): Path<(String, String)>,
    State(state): State<Arc<AppState>>,
) -> Result<Response, ApiError> {
	let instance_id = resolve_upload_instance_id(&state, &claims.sub, &id)
		.await?;
	cancel_upload(&state, &instance_id, &upload_id)
		.await
		.map_err(upload_error)?;
	Ok(StatusCode::NO_CONTENT.into_response())
}

async fn resolve_upload_instance_id(
	state: &Arc<AppState>,
	user_id: &str,
	path: &str,
) -> Result<String, ApiError> {
	Ok(resolve_authorized_instance_id(state, user_id, path, "server:files")
		.await?
		.to_string())
}

fn encode_path_segment(value: &str) -> String {
	let mut encoded = String::new();
	for byte in value.as_bytes() {
		match *byte {
			b'A'..=b'Z'
			| b'a'..=b'z'
			| b'0'..=b'9'
			| b'-'
			| b'.'
			| b'_'
			| b'~' => encoded.push(*byte as char),
			other => encoded.push_str(&format!("%{other:02X}")),
		}
	}
	encoded
}

fn parse_required_u64(
    headers: &HeaderMap,
    name: &HeaderName,
) -> Result<u64, ApiError> {
    let value = headers
        .get(name)
        .ok_or_else(|| ApiError::BadRequest(format!("missing {name} header")))?
        .to_str()
        .map_err(|_| ApiError::BadRequest(format!("invalid {name} header")))?;
    value
        .parse::<u64>()
        .map_err(|_| ApiError::BadRequest(format!("invalid {name} header")))
}

fn upload_metadata(
    headers: &HeaderMap,
    key: &str,
) -> Result<Option<String>, ApiError> {
    let Some(value) = headers.get(&UPLOAD_METADATA) else {
        return Ok(None);
    };
    for item in value
        .to_str()
        .map_err(|_| ApiError::BadRequest("invalid upload metadata".into()))?
        .split(',')
    {
        let mut parts = item.trim().splitn(2, ' ');
        let Some(item_key) = parts.next() else {
            continue;
        };
        let Some(encoded) = parts.next() else {
            continue;
        };
        if item_key == key {
            let bytes = STANDARD.decode(encoded).map_err(|_| {
                ApiError::BadRequest("invalid upload metadata".into())
            })?;
            let value = String::from_utf8(bytes).map_err(|_| {
                ApiError::BadRequest("invalid upload metadata".into())
            })?;
            return Ok(Some(value));
        }
    }
    Ok(None)
}

fn upload_checksum(headers: &HeaderMap) -> Result<Option<String>, ApiError> {
    let Some(value) = headers.get(&UPLOAD_CHECKSUM) else {
        return Ok(None);
    };
    let value = value
        .to_str()
        .map_err(|_| ApiError::BadRequest("invalid upload checksum".into()))?;
    let Some(encoded) = value.strip_prefix("sha256 ") else {
        return Err(ApiError::BadRequest(
            "only sha256 upload checksum is supported".into(),
        ));
    };
    let bytes = STANDARD
        .decode(encoded)
        .map_err(|_| ApiError::BadRequest("invalid upload checksum".into()))?;
    Ok(Some(hex::encode(bytes)))
}

fn header_value(value: &str) -> Result<HeaderValue, ApiError> {
    HeaderValue::from_str(value)
        .map_err(|_| ApiError::Internal("invalid response header".into()))
}

fn upload_error(error: UploadError) -> ApiError {
    match error {
        UploadError::NotFound | UploadError::UploadNotFound => {
            ApiError::NotFound("upload not found".into())
        }
        UploadError::PathTraversal
        | UploadError::Invalid(_)
        | UploadError::OffsetMismatch
        | UploadError::ChecksumMismatch => {
            ApiError::BadRequest(error.to_string())
        }
        UploadError::Io(error) => ApiError::Internal(error.to_string()),
        UploadError::Db(error) => ApiError::Internal(error.to_string()),
    }
}
