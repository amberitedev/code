const SHARED_INSTANCE_INVITE_MAX_AGE_SECONDS: i32 = 604800;
const SHARED_INSTANCE_INVITE_MAX_USES: i32 = 10;

use super::client::*;
use super::install::*;
use super::publish::*;
use super::types::*;
use super::*;

#[tracing::instrument(skip(state))]
pub async fn get_shared_instance_users(
    state: &State,
    instance_id: &str,
) -> crate::Result<SharedInstanceUsers> {
    let Some(attachment) = shared_attachment(instance_id, state).await? else {
        return Ok(SharedInstanceUsers::empty());
    };

    get_remote_users(&attachment.id, state).await
}

#[tracing::instrument(skip(state))]
pub async fn invite_shared_instance_users(
    state: &State,
    instance_id: &str,
    user_ids: Vec<String>,
) -> crate::Result<SharedInstanceUsers> {
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let (metadata, attachment) =
        shared_instance_for_invites(instance_id, user_ids.len(), state).await?;

    ensure_owner(&attachment)?;
    if !user_ids.is_empty() {
        ensure_ready_remote_version_for_invite(instance_id, &attachment, state)
            .await?;
        update_remote_instance(
            &attachment.id,
            shared_instance_name(metadata.instance.name.clone()),
            state,
        )
        .await?;
        tracing::info!(
            instance_id,
            shared_instance_id = %attachment.id,
            user_count = user_ids.len(),
            "Adding users to shared instance"
        );
        add_remote_users(&attachment.id, user_ids.clone(), state).await?;
    }
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    get_remote_users(&attachment.id, state).await
}

#[tracing::instrument(skip(state, replace_invite_id))]
pub async fn create_shared_instance_invite_link(
    state: &State,
    instance_id: &str,
    max_age_seconds: Option<i32>,
    max_uses: Option<i32>,
    replace_invite_id: Option<String>,
) -> crate::Result<SharedInstanceInviteLink> {
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let (metadata, attachment) =
        shared_instance_for_invites(instance_id, 0, state).await?;
    ensure_owner(&attachment)?;
    ensure_ready_remote_version_for_invite(instance_id, &attachment, state)
        .await?;
    update_remote_instance(
        &attachment.id,
        shared_instance_name(metadata.instance.name),
        state,
    )
    .await?;

    let max_age_seconds =
        max_age_seconds.unwrap_or(SHARED_INSTANCE_INVITE_MAX_AGE_SECONDS);
    let max_uses = max_uses.unwrap_or(SHARED_INSTANCE_INVITE_MAX_USES);
    if max_age_seconds <= 0
        || max_age_seconds > SHARED_INSTANCE_INVITE_MAX_AGE_SECONDS
    {
        bail!(crate::ErrorKind::InputError(
            "invite expiry must be between now and seven days".to_string(),
        ));
    }
    if max_uses <= 0 {
        bail!(crate::ErrorKind::InputError(
            "invite max uses must be greater than zero".to_string(),
        ));
    }

    let created_at = Utc::now();

    let invite_id =
        create_remote_invite(&attachment.id, max_age_seconds, max_uses, state)
            .await?;

    // Amberite: the replaced link goes only once its replacement exists, and a failed delete takes
    // the new one back, so a failure leaves the old link working. The Play tab replaces server
    // links the same way (packages/ui use-server-players.ts).
    if let Some(replaced) = replace_invite_id
        && let Err(error) =
            delete_remote_invite(&attachment.id, &replaced, state).await
    {
        if let Err(undo) =
            delete_remote_invite(&attachment.id, &invite_id, state).await
        {
            tracing::warn!(%undo, "could not take back a replacement invite link");
        }
        return Err(error);
    }

    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(SharedInstanceInviteLink {
        invite_id,
        expires_at: created_at
            + chrono::Duration::seconds(i64::from(max_age_seconds)),
        max_uses,
    })
}

#[tracing::instrument(skip(state))]
pub async fn get_shared_instance_invites(
    state: &State,
    instance_id: &str,
) -> crate::Result<Vec<SharedInstanceInvite>> {
    let Some(attachment) = shared_attachment(instance_id, state).await? else {
        return Ok(Vec::new());
    };
    ensure_owner(&attachment)?;

    Ok(get_remote_invites(&attachment.id, state)
        .await?
        .into_iter()
        .map(|invite| SharedInstanceInvite {
            id: invite.id,
            expiration: invite.expiration,
            max_uses: invite.max_uses,
            uses: invite.uses,
        })
        .collect())
}

#[tracing::instrument(skip(state, invite_id))]
pub async fn revoke_shared_instance_invite(
    state: &State,
    instance_id: &str,
    invite_id: String,
) -> crate::Result<()> {
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let Some(attachment) = shared_attachment(instance_id, state).await? else {
        return Ok(());
    };
    ensure_owner(&attachment)?;

    delete_remote_invite(&attachment.id, &invite_id, state).await?;
    emit_instance(&state.events, instance_id, InstancePayloadType::Edited).await
}

#[tracing::instrument(skip(state))]
pub async fn remove_shared_instance_users(
    state: &State,
    instance_id: &str,
    user_ids: Vec<String>,
    has_pending_recipients: bool,
) -> crate::Result<SharedInstanceUsers> {
    let _shared_instance_lock =
        state.instance_locks.lock_shared(instance_id).await;
    let Some(attachment) = shared_attachment(instance_id, state).await? else {
        return Ok(SharedInstanceUsers::empty());
    };
    ensure_owner(&attachment)?;

    if !user_ids.is_empty() {
        remove_remote_users(&attachment.id, user_ids, state).await?;
    }

    let remaining_users = get_remote_users(&attachment.id, state).await?;
    if !has_shared_instance_recipients(
        &remaining_users,
        &attachment,
        has_pending_recipients,
        state,
    )
    .await?
    {
        delete_remote_instance(&attachment.id, state).await?;
        crate::state::clear_shared_instance(instance_id, &state.pool).await?;
        emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
            .await?;

        return Ok(SharedInstanceUsers::empty());
    }

    emit_instance(&state.events, instance_id, InstancePayloadType::Edited)
        .await?;

    Ok(remaining_users)
}

#[tracing::instrument(skip(state))]
pub async fn accept_pending_shared_instance_invite(
    state: &State,
    shared_instance_id: &str,
) -> crate::Result<()> {
    // Read access doesn't imply membership: staff and server members can read
    // an instance while their invite is still pending.
    let has_access =
        match get_remote_instance_access(shared_instance_id, state).await? {
            SharedInstanceRemoteResponse::Available(_) => true,
            SharedInstanceRemoteResponse::Unavailable(
                SharedInstanceUnavailableReason::AccessRevoked,
            ) => false,
            SharedInstanceRemoteResponse::Unavailable(reason) => {
                return Err(shared_instance_unavailable_error(reason));
            }
        };

    if accept_pending_remote_invite(shared_instance_id, state).await?
        || has_access
    {
        return Ok(());
    }

    bail!(crate::ErrorKind::InputError(
        "no pending invite found for shared instance".to_string(),
    ))
}

#[tracing::instrument(skip(state))]
pub async fn decline_pending_shared_instance_invite(
    state: &State,
    shared_instance_id: &str,
) -> crate::Result<()> {
    decline_pending_remote_invite(shared_instance_id, state).await
}
