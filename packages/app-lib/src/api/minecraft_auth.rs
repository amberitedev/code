//! Authentication flow interface

use reqwest::StatusCode;

use crate::State;
pub use crate::state::{
    AmberiteNativeSession, MinecraftAccountSummary, RememberedAmberiteIdentity,
    StagedMinecraftLogin,
};
use crate::state::{Credentials, MinecraftLoginFlow};
use crate::util::fetch::INSECURE_REQWEST_CLIENT;

pub fn configure_product_session_account(account: impl Into<String>) {
    crate::state::configure_product_session_account(account);
}

#[tracing::instrument]
pub async fn check_reachable() -> crate::Result<()> {
    let resp = INSECURE_REQWEST_CLIENT
        .get("https://sessionserver.mojang.com/session/minecraft/hasJoined")
        .send()
        .await?;
    if resp.status() == StatusCode::NO_CONTENT {
        return Ok(());
    }
    resp.error_for_status()?;
    Ok(())
}

#[tracing::instrument]
pub async fn begin_login() -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::login_begin(&state.pool).await
}

#[tracing::instrument]
pub async fn begin_login_with_prompt(
    select_account: bool,
) -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::login_begin_with_prompt(select_account, &state.pool).await
}

#[tracing::instrument]
pub async fn begin_login_staged_with_prompt(
    select_account: bool,
) -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::login_begin_staged_with_prompt(select_account, &state.pool)
        .await
}

#[tracing::instrument]
pub async fn finish_login(
    code: &str,
    flow: MinecraftLoginFlow,
) -> crate::Result<Credentials> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::login_finish(code, flow, &state.pool).await
}

#[tracing::instrument]
pub async fn finish_login_staged(
    code: &str,
    flow: MinecraftLoginFlow,
) -> crate::Result<StagedMinecraftLogin> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::login_finish_staged(code, flow, &state.pool).await
}

pub async fn amberite_product_session()
-> crate::Result<Option<AmberiteNativeSession>> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::amberite_product_session(&state.pool).await
}

pub async fn remembered_amberite_identity()
-> crate::Result<Option<RememberedAmberiteIdentity>> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::remembered_amberite_identity(&state.pool).await
}

pub async fn commit_amberite_product_session(
    staged: StagedMinecraftLogin,
    expected_minecraft_uuid: Option<uuid::Uuid>,
    session: AmberiteNativeSession,
    remembered_identity: RememberedAmberiteIdentity,
) -> crate::Result<()> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    crate::state::commit_amberite_product_session(
        &state.pool,
        staged,
        expected_minecraft_uuid,
        session,
        remembered_identity,
    )
    .await
}

pub async fn attach_legacy_amberite_product_session(
    credentials: Credentials,
    session: AmberiteNativeSession,
    remembered_identity: RememberedAmberiteIdentity,
) -> crate::Result<()> {
    let state = State::get().await?;
    crate::state::attach_legacy_amberite_product_session(
        &state.pool,
        credentials,
        session,
        remembered_identity,
    )
    .await
}

pub async fn update_amberite_product_session(
    session: AmberiteNativeSession,
) -> crate::Result<()> {
    let state = State::get().await?;
    crate::state::update_amberite_product_session(&state.pool, session).await
}

pub async fn clear_product_session_preserving_identity() -> crate::Result<()> {
    let state = State::get().await?;
    crate::state::clear_product_session_preserving_identity(&state.pool).await
}

#[tracing::instrument]
pub async fn get_default_user() -> crate::Result<Option<uuid::Uuid>> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    let user = Credentials::get_active(&state.pool).await?;
    Ok(user.map(|user| user.offline_profile.id))
}

#[tracing::instrument]
pub async fn set_default_user(user: uuid::Uuid) -> crate::Result<()> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    let users = Credentials::get_all(&state.pool).await?;
    let (_, mut user) = users.remove(&user).ok_or_else(|| {
        crate::ErrorKind::OtherError(format!(
            "Tried to get nonexistent user with ID {user}"
        ))
        .as_error()
    })?;

    user.active = true;
    user.upsert(&state.pool).await?;

    Ok(())
}

/// Remove a user account from the database
#[tracing::instrument]
pub async fn remove_user(uuid: uuid::Uuid) -> crate::Result<()> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;

    let users = Credentials::get_all(&state.pool).await?;

    if let Some((uuid, user)) = users.remove(&uuid) {
        Credentials::remove(uuid, &state.pool).await?;

        if user.active
            && let Some((_, mut user)) = users.into_iter().next()
        {
            user.active = true;
            user.upsert(&state.pool).await?;
        }
    }

    Ok(())
}

/// Get a copy of the list of all user credentials
#[tracing::instrument]
pub async fn users() -> crate::Result<Vec<MinecraftAccountSummary>> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    let users = Credentials::get_all(&state.pool).await?;
    let mut summaries = Vec::with_capacity(users.len());
    for (_, credentials) in users {
        summaries.push(credentials.account_summary().await);
    }
    Ok(summaries)
}

/// Get the active/default Minecraft credentials, refreshing them if needed.
#[tracing::instrument]
pub async fn default_credential() -> crate::Result<Option<Credentials>> {
    let state = State::get().await?;
    crate::state::migrate_legacy_product_session(&state.pool).await?;
    Credentials::get_default_credential(&state.pool).await
}
