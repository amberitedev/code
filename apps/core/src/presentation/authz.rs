use std::sync::Arc;

use crate::{
    application::{access_service, state::AppState},
    presentation::error::ApiError,
};

pub async fn require_instance_permission(
    state: &Arc<AppState>,
    user_id: &str,
    instance_id: &str,
    permission: &str,
) -> Result<(), ApiError> {
    access_service::require_instance_permission(
        state,
        user_id,
        instance_id,
        permission,
    )
    .await
    .map(|_| ())
    .map_err(|error| ApiError::Forbidden(error.to_string()))
}

pub async fn require_core_member(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<(), ApiError> {
    access_service::require_core_member(state, user_id)
        .await
        .map(|_| ())
        .map_err(|error| ApiError::Forbidden(error.to_string()))
}

pub async fn require_core_manager(
    state: &Arc<AppState>,
    user_id: &str,
) -> Result<(), ApiError> {
    access_service::require_core_manager(state, user_id)
        .await
        .map(|_| ())
        .map_err(|error| ApiError::Forbidden(error.to_string()))
}

pub async fn can_access_instance(
    state: &Arc<AppState>,
    user_id: &str,
    instance_id: &str,
    permission: &str,
) -> bool {
    access_service::require_instance_permission(
        state,
        user_id,
        instance_id,
        permission,
    )
    .await
    .is_ok()
}
