use crate::state::{FeatureFlag, ModrinthCredentials, Settings};
use anyhow::bail;
use serde::Deserialize;

const LOCALHOST_LOGIN_URL: &str = "http://localhost:3000/auth/sign-in";
const LOCALHOST_SIGNUP_URL: &str = "http://localhost:3000/auth/sign-up";

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ModrinthAuthFlow {
    SignIn,
    SignUp,
}

#[tracing::instrument]
pub async fn authenticate_begin_flow(
    flow: ModrinthAuthFlow,
) -> crate::Result<String> {
    let state = crate::State::get().await?;
    if let Some(url) = crate::self_hosted::accounts::login_url(
        &state.sessions,
        matches!(flow, ModrinthAuthFlow::SignUp),
    ) {
        return Ok(url);
    }
    let settings = Settings::get(&state.pool).await?;
    let use_localhost = settings
        .feature_flags
        .get(&FeatureFlag::LocalhostSignIn)
        .copied()
        .unwrap_or(false);

    Ok(match (use_localhost, flow) {
        (true, ModrinthAuthFlow::SignIn) => LOCALHOST_LOGIN_URL,
        (true, ModrinthAuthFlow::SignUp) => LOCALHOST_SIGNUP_URL,
        (false, ModrinthAuthFlow::SignIn) => crate::state::get_login_url(),
        (false, ModrinthAuthFlow::SignUp) => crate::state::get_signup_url(),
    }
    .to_owned())
}

#[tracing::instrument(skip(code))]
pub async fn authenticate_finish_flow(
    code: &str,
) -> crate::Result<ModrinthCredentials> {
    let state = crate::State::get().await?;

    let creds = crate::state::finish_login_flow(
        code,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;

    creds.upsert(&state.pool).await?;
    sqlx::query("INSERT INTO self_hosted_account_origins (user_id,origin) VALUES (?,?) ON CONFLICT(user_id) DO UPDATE SET origin=excluded.origin")
        .bind(&creds.user_id).bind(crate::self_hosted::accounts::origin(&state.sessions)).execute(&state.pool).await?;
    crate::self_hosted::accounts::set_sessions(&state.sessions, Some(&creds));

    if let Err(error) =
        crate::onboarding_checklist::mark_logged_into_modrinth().await
    {
        tracing::warn!(
            "Failed to mark Modrinth login in onboarding checklist: {error}"
        );
    }

    reconnect_friends(&state).await?;

    Ok(creds)
}

#[tracing::instrument]
pub async fn logout() -> crate::Result<()> {
    let state = crate::State::get().await?;
    sign_out(&state).await
}

async fn sign_out(state: &crate::State) -> crate::Result<()> {
    ModrinthCredentials::deactivate_all(&state.pool).await?;
    crate::self_hosted::accounts::set_sessions(&state.sessions, None);
    state.presence.friends_socket.disconnect().await?;

    Ok(())
}

#[tracing::instrument]
pub async fn get_all() -> crate::Result<Vec<ModrinthCredentials>> {
    let state = crate::State::get().await?;
    ModrinthCredentials::get_all(&state.pool, &state.sessions).await
}

#[tracing::instrument]
pub async fn set_active(user_id: &str) -> crate::Result<()> {
    let state = crate::State::get().await?;
    let users =
        ModrinthCredentials::get_all(&state.pool, &state.sessions).await?;
    let Some(mut creds) =
        users.into_iter().find(|creds| creds.user_id == user_id)
    else {
        bail!("tried to activate nonexistent Modrinth user with ID {user_id}");
    };

    creds.active = true;
    creds.upsert(&state.pool).await?;
    crate::self_hosted::accounts::set_sessions(&state.sessions, Some(&creds));
    reconnect_friends(&state).await?;

    Ok(())
}

#[tracing::instrument]
pub async fn remove_user(user_id: &str) -> crate::Result<()> {
    let state = crate::State::get().await?;
    if let Some(creds) =
        ModrinthCredentials::get_all(&state.pool, &state.sessions)
            .await?
            .into_iter()
            .find(|creds| creds.user_id == user_id)
    {
        if let Err(error) = crate::self_hosted::accounts::revoke(
            &creds,
            &state.api_semaphore,
            &state.sessions,
        )
        .await
        {
            tracing::warn!("Could not revoke account session: {error}");
        }
    }
    ModrinthCredentials::remove(user_id, &state.pool).await?;

    if ModrinthCredentials::get_active(&state.pool)
        .await?
        .is_none()
    {
        crate::self_hosted::accounts::set_sessions(&state.sessions, None);
        state.presence.friends_socket.disconnect().await?;
    }

    Ok(())
}

async fn reconnect_friends(state: &crate::State) -> crate::Result<()> {
    if let Err(error) = state.presence.friends_socket.disconnect().await {
        tracing::warn!("Failed to disconnect friends socket: {error}");
    }
    if let Err(error) = state
        .presence
        .friends_socket
        .connect(
            &state.pool,
            &state.api_semaphore,
            &state.sessions,
            &state.process_manager,
            &state.events,
        )
        .await
    {
        tracing::warn!("Failed to reconnect friends socket: {error}");
    }

    Ok(())
}

#[tracing::instrument]
pub async fn get_credentials() -> crate::Result<Option<ModrinthCredentials>> {
    let state = crate::State::get().await?;
    let current = ModrinthCredentials::get_and_refresh(
        &state.pool,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;
    if current.is_none() {
        state.presence.friends_socket.disconnect().await?;
    }

    crate::self_hosted::accounts::set_sessions(
        &state.sessions,
        current.as_ref(),
    );
    Ok(current)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test(flavor = "multi_thread")]
    async fn signing_out_clears_the_sessions() {
        crate::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let accounts = crate::self_hosted::accounts::AccountEndpoints::new(
            "https://accounts.example/",
            None,
        )
        .unwrap();
        let state = crate::State::new(
            dir.path().to_path_buf(),
            crate::event::EventSink::new(),
            Some(accounts),
        )
        .await
        .unwrap();
        let credentials = ModrinthCredentials {
            session: "session".into(),
            expires: chrono::Utc::now() + chrono::Duration::days(1),
            user_id: "user".into(),
            active: true,
        };
        credentials.upsert(&state.pool).await.unwrap();
        crate::self_hosted::accounts::set_sessions(
            &state.sessions,
            Some(&credentials),
        );

        sign_out(&state).await.unwrap();

        assert!(
            state
                .sessions
                .modrinth_token("https://accounts.example/v2/user")
                .is_none()
        );
        assert!(state.sessions.backend().is_none());
    }
}
