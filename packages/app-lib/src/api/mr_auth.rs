use crate::state::ModrinthCredentials;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModrinthAuthFlow {
    SignIn,
    SignUp,
}

#[tracing::instrument]
pub fn authenticate_begin_flow(flow: ModrinthAuthFlow) -> String {
    if let Some(url) = crate::self_hosted::accounts::login_url(matches!(
        flow,
        ModrinthAuthFlow::SignUp
    )) {
        return url;
    }
    match flow {
        ModrinthAuthFlow::SignIn => crate::state::get_login_url().to_owned(),
        ModrinthAuthFlow::SignUp => crate::state::get_signup_url().to_owned(),
    }
}

#[tracing::instrument(skip(code))]
pub async fn authenticate_finish_flow(
    code: &str,
) -> crate::Result<ModrinthCredentials> {
    let state = crate::State::get().await?;

    let creds = crate::state::finish_login_flow(
        code,
        &state.api_semaphore,
        &state.pool,
    )
    .await?;

    creds.upsert(&state.pool).await?;
    crate::self_hosted::accounts::sync_sharing(Some(&creds));

    if let Err(error) =
        crate::onboarding_checklist::mark_logged_into_modrinth().await
    {
        tracing::warn!(
            "Failed to mark Modrinth login in onboarding checklist: {error}"
        );
    }

    state.friends_socket.disconnect().await?;
    state
        .friends_socket
        .connect(&state.pool, &state.api_semaphore, &state.process_manager)
        .await?;

    Ok(creds)
}

#[tracing::instrument]
pub async fn logout() -> crate::Result<()> {
    let state = crate::State::get().await?;
    let current = ModrinthCredentials::get_active(&state.pool).await?;

    if let Some(current) = current {
        if let Err(error) = crate::self_hosted::accounts::revoke(
            &current,
            &state.api_semaphore,
            &state.pool,
        )
        .await
        {
            tracing::warn!("Could not revoke account session: {error}");
        }
        ModrinthCredentials::remove(&current.user_id, &state.pool).await?;
    }
    state.friends_socket.disconnect().await?;
    crate::self_hosted::accounts::sync_sharing(None);

    Ok(())
}

#[tracing::instrument]
pub async fn get_credentials() -> crate::Result<Option<ModrinthCredentials>> {
    let state = crate::State::get().await?;
    let current =
        ModrinthCredentials::get_and_refresh(&state.pool, &state.api_semaphore)
            .await?;
    if current.is_none() {
        state.friends_socket.disconnect().await?;
    }

    crate::self_hosted::accounts::sync_sharing(current.as_ref());
    Ok(current)
}
