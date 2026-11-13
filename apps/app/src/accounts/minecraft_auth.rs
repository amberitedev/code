//! Authentication flow interface

use anyhow::Context;
use reqwest::StatusCode;

pub use crate::accounts::state::MinecraftAccountSummary;
use crate::accounts::state::{Credentials, MinecraftLoginFlow};
use engine::Engine;
use engine::INSECURE_REQWEST_CLIENT;

#[tracing::instrument]
pub async fn check_reachable() -> engine::Result<()> {
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

#[tracing::instrument(skip(state))]
pub async fn begin_login(state: &Engine) -> engine::Result<MinecraftLoginFlow> {
    crate::accounts::state::login_begin(state.pool()).await
}

#[tracing::instrument(skip_all)]
pub async fn finish_login(
    state: &Engine,
    code: &str,
    flow: MinecraftLoginFlow,
) -> engine::Result<Credentials> {
    let credentials =
        crate::accounts::state::login_finish(code, flow, state.pool()).await?;
    mark_logged_into_minecraft(state).await;
    Ok(credentials)
}

async fn mark_logged_into_minecraft(state: &engine::Engine) {
    if let Err(error) =
        engine::onboarding_checklist::mark_logged_into_minecraft(state).await
    {
        tracing::warn!(
            "Failed to mark Minecraft login in onboarding checklist: {error}"
        );
    }
}

#[tracing::instrument(skip(state))]
pub async fn get_default_user(
    state: &Engine,
) -> engine::Result<Option<uuid::Uuid>> {
    let user = Credentials::get_default_credential(state.pool()).await?;
    Ok(user.map(|user| user.offline_profile.id))
}

#[tracing::instrument(skip(state))]
pub async fn set_default_user(
    state: &Engine,
    user: uuid::Uuid,
) -> engine::Result<()> {
    let users = Credentials::get_all(state.pool()).await?;
    let (_, mut user) = users
        .remove(&user)
        .context(format!("tried to get nonexistent user with ID {user}"))?;

    user.active = true;
    user.upsert(state.pool()).await?;

    Ok(())
}

/// Remove a user account from the database
#[tracing::instrument(skip(state))]
pub async fn remove_user(
    state: &Engine,
    uuid: uuid::Uuid,
) -> engine::Result<()> {
    let users = Credentials::get_all(state.pool()).await?;

    if let Some((uuid, user)) = users.remove(&uuid) {
        Credentials::remove(uuid, state.pool()).await?;

        if user.active
            && let Some((_, mut user)) = users.into_iter().next()
        {
            user.active = true;
            user.upsert(state.pool()).await?;
        }
    }

    Ok(())
}

/// Get a copy of the list of all user credentials
#[tracing::instrument(skip(state))]
pub async fn users(
    state: &Engine,
) -> engine::Result<Vec<MinecraftAccountSummary>> {
    let users = Credentials::get_all(state.pool()).await?;
    let mut summaries = Vec::with_capacity(users.len());
    for (_, credentials) in users {
        summaries.push(credentials.account_summary().await);
    }
    Ok(summaries)
}

/// Get the active/default Minecraft credentials, refreshing them if needed.
#[tracing::instrument(skip(state))]
pub async fn default_credential(
    state: &Engine,
) -> engine::Result<Option<Credentials>> {
    Credentials::get_default_credential(state.pool()).await
}
