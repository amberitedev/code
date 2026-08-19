//! Authentication flow interface

use reqwest::StatusCode;

use crate::State;
use crate::state::{Credentials, MinecraftLoginFlow};
pub use crate::state::{MinecraftAccountSummary, StagedMinecraftLogin};
use crate::util::fetch::INSECURE_REQWEST_CLIENT;

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
    crate::state::login_begin(&state.pool).await
}

#[tracing::instrument]
pub async fn begin_login_with_prompt(
    select_account: bool,
) -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::login_begin_with_prompt(select_account, &state.pool).await
}

#[tracing::instrument]
pub async fn begin_login_staged_with_prompt(
    select_account: bool,
) -> crate::Result<MinecraftLoginFlow> {
    let state = State::get().await?;
    crate::state::login_begin_staged_with_prompt(select_account, &state.pool)
        .await
}

#[tracing::instrument(skip_all)]
pub async fn finish_login(
    code: &str,
    flow: MinecraftLoginFlow,
) -> crate::Result<Credentials> {
    let state = State::get().await?;
    let credentials =
        crate::state::login_finish(code, flow, &state.pool).await?;
    mark_logged_into_minecraft().await;
    Ok(credentials)
}

#[tracing::instrument(skip_all)]
pub async fn finish_login_staged(
    code: &str,
    flow: MinecraftLoginFlow,
) -> crate::Result<StagedMinecraftLogin> {
    let state = State::get().await?;
    crate::state::login_finish_staged(code, flow, &state.pool).await
}

pub async fn commit_staged_login(
    staged: StagedMinecraftLogin,
) -> crate::Result<Credentials> {
    let state = State::get().await?;
    let credentials = staged.commit(&state.pool).await?;
    mark_logged_into_minecraft().await;
    Ok(credentials)
}

async fn mark_logged_into_minecraft() {
    if let Err(error) =
        crate::onboarding_checklist::mark_logged_into_minecraft().await
    {
        tracing::warn!(
            "Failed to mark Minecraft login in onboarding checklist: {error}"
        );
    }
}

#[tracing::instrument]
pub async fn get_default_user() -> crate::Result<Option<uuid::Uuid>> {
    let state = State::get().await?;
    let user = Credentials::get_default_credential(&state.pool).await?;
    Ok(user.map(|user| user.offline_profile.id))
}

#[tracing::instrument]
pub async fn set_default_user(user: uuid::Uuid) -> crate::Result<()> {
    let state = State::get().await?;
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
    Credentials::get_default_credential(&state.pool).await
}
