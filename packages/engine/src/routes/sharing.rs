//! Publishing itself is an App op, in `apps/app/src/library/sharing.rs`.

use std::sync::Arc;

use axum::extract::State;
use contracts::{ApiError, Empty, Json, server::Req};

use super::api_error;
use crate::{
    Engine,
    access::{Edit, EngineAccess, InstanceAccess, Owner, View},
    endpoints::{
        instances::SharedInstanceAttachment,
        sharing::{
            CreateSharedInstanceInviteLink,
            DismissSharedInstanceRestoredNotice, EnsureSharedInstance,
            GetSharedInstancePublishPreview,
            GetSharedInstancePublishSupportDetails, GetSharedInstanceUsers,
            InviteSharedInstanceUsers, LeaveRemoteSharedInstance,
            ListSharedInstanceInvites, RemoveSharedInstanceUsers,
            RevokeSharedInstanceInvite, UnlinkSharedInstance,
            UnpublishRemoteSharedInstance, UnpublishSharedInstance,
        },
    },
    instance::{
        self, SharedInstanceInvite, SharedInstanceInviteLink,
        SharedInstancePublishPreview, SharedInstanceUsers,
    },
};

pub async fn get_shared_instance_users(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<View>,
    _: Req<GetSharedInstanceUsers>,
) -> Result<Json<SharedInstanceUsers>, ApiError> {
    instance::get_shared_instance_users(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn invite_shared_instance_users(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    request: Req<InviteSharedInstanceUsers>,
) -> Result<Json<SharedInstanceUsers>, ApiError> {
    instance::invite_shared_instance_users(
        &engine,
        &instance.id,
        request.body.user_ids,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn remove_shared_instance_users(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    request: Req<RemoveSharedInstanceUsers>,
) -> Result<Json<SharedInstanceUsers>, ApiError> {
    instance::remove_shared_instance_users(
        &engine,
        &instance.id,
        request.body.user_ids,
        request.body.has_pending_recipients,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn list_shared_instance_invites(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<ListSharedInstanceInvites>,
) -> Result<Json<Vec<SharedInstanceInvite>>, ApiError> {
    instance::get_shared_instance_invites(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn create_shared_instance_invite_link(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    request: Req<CreateSharedInstanceInviteLink>,
) -> Result<Json<SharedInstanceInviteLink>, ApiError> {
    let link = request.body;
    instance::create_shared_instance_invite_link(
        &engine,
        &instance.id,
        link.max_age_seconds,
        link.max_uses,
        link.replace_invite_id,
    )
    .await
    .map(Json)
    .map_err(api_error)
}

pub async fn revoke_shared_instance_invite(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    request: Req<RevokeSharedInstanceInvite>,
) -> Result<Empty, ApiError> {
    instance::revoke_shared_instance_invite(
        &engine,
        &instance.id,
        request.params.invite_id,
    )
    .await
    .map(|()| Empty)
    .map_err(api_error)
}

pub async fn get_shared_instance_publish_preview(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<GetSharedInstancePublishPreview>,
) -> Result<Json<Option<SharedInstancePublishPreview>>, ApiError> {
    instance::get_shared_instance_publish_preview(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn get_shared_instance_publish_support_details(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<GetSharedInstancePublishSupportDetails>,
) -> Result<Json<Option<String>>, ApiError> {
    instance::get_shared_instance_publish_support_details(&engine, &instance.id)
        .await
        .map(Json)
        .map_err(api_error)
}

pub async fn ensure_shared_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<EnsureSharedInstance>,
) -> Result<Json<SharedInstanceAttachment>, ApiError> {
    instance::ensure_shared_instance(&engine, &instance.id)
        .await
        .map(|attachment| Json(attachment.into()))
        .map_err(api_error)
}

pub async fn unpublish_shared_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<UnpublishSharedInstance>,
) -> Result<Empty, ApiError> {
    instance::unpublish_shared_instance(&engine, &instance.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn unlink_shared_instance(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Owner>,
    _: Req<UnlinkSharedInstance>,
) -> Result<Empty, ApiError> {
    instance::unlink_shared_instance(&engine, &instance.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn dismiss_shared_instance_restored_notice(
    State(engine): State<Arc<Engine>>,
    instance: InstanceAccess<Edit>,
    _: Req<DismissSharedInstanceRestoredNotice>,
) -> Result<Empty, ApiError> {
    instance::dismiss_shared_instance_restored_notice(&engine, &instance.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn unpublish_remote_shared_instance(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<UnpublishRemoteSharedInstance>,
) -> Result<Empty, ApiError> {
    instance::unpublish_remote_shared_instance(&engine, &request.params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}

pub async fn leave_remote_shared_instance(
    State(engine): State<Arc<Engine>>,
    _: EngineAccess<Owner>,
    request: Req<LeaveRemoteSharedInstance>,
) -> Result<Empty, ApiError> {
    instance::leave_remote_shared_instance(&engine, &request.params.id)
        .await
        .map(|()| Empty)
        .map_err(api_error)
}
