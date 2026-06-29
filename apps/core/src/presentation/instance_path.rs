use std::sync::Arc;

use crate::{
	application::{access_service, state::AppState},
	domain::instance::{InstanceId, InstanceRecord},
	ports::instance_store::StoreError,
	presentation::error::ApiError,
};

pub async fn resolve_instance_path(
    state: &Arc<AppState>,
    path: &str,
) -> Result<InstanceRecord, ApiError> {
	if path.trim().is_empty() {
		return Err(ApiError::BadRequest("instance path cannot be empty".into()));
	}
	match state.instance_store.get_by_path(path).await {
		Ok(record) => Ok(record),
		Err(StoreError::NotFound(_)) => {
			let Ok(id) = path.parse::<InstanceId>() else {
				return Err(ApiError::NotFound(format!("instance {path} not found")));
			};
			Ok(state.instance_store.get(&id).await?)
		}
		Err(error) => Err(error.into()),
	}
}

pub async fn resolve_instance_id(
    state: &Arc<AppState>,
    path: &str,
) -> Result<InstanceId, ApiError> {
    Ok(resolve_instance_path(state, path).await?.id)
}

pub async fn resolve_authorized_instance(
    state: &Arc<AppState>,
    user_id: &str,
    path: &str,
    permission: &str,
) -> Result<InstanceRecord, ApiError> {
    let record = resolve_instance_path(state, path).await?;
    access_service::require_instance_permission(
        state,
        user_id,
        &record.id.to_string(),
        permission,
    )
    .await
    .map(|_| record)
    .map_err(|error| ApiError::Forbidden(error.to_string()))
}

pub async fn resolve_authorized_instance_id(
    state: &Arc<AppState>,
    user_id: &str,
    path: &str,
    permission: &str,
) -> Result<InstanceId, ApiError> {
    Ok(resolve_authorized_instance(state, user_id, path, permission)
        .await?
        .id)
}
