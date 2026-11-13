use crate::accounts::state::ModrinthCredentials;
use crate::presence::Presence;
use anyhow::bail;
use engine::data::{FeatureFlag, Settings};
use serde::Deserialize;

const LOCALHOST_LOGIN_URL: &str = "http://localhost:3000/auth/sign-in";
const LOCALHOST_SIGNUP_URL: &str = "http://localhost:3000/auth/sign-up";

#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ModrinthAuthFlow {
    SignIn,
    SignUp,
}

#[tracing::instrument(skip(state))]
pub async fn authenticate_begin_flow(
    state: &engine::Engine,
    flow: ModrinthAuthFlow,
) -> engine::Result<String> {
    if let Some(url) = engine::self_hosted::accounts::login_url(
        &state.sessions,
        matches!(flow, ModrinthAuthFlow::SignUp),
    ) {
        return Ok(url);
    }
    let settings = Settings::get(state.pool()).await?;
    let use_localhost = settings
        .feature_flags
        .get(&FeatureFlag::LocalhostSignIn)
        .copied()
        .unwrap_or(false);

    Ok(match (use_localhost, flow) {
        (true, ModrinthAuthFlow::SignIn) => LOCALHOST_LOGIN_URL,
        (true, ModrinthAuthFlow::SignUp) => LOCALHOST_SIGNUP_URL,
        (false, ModrinthAuthFlow::SignIn) => {
            crate::accounts::state::get_login_url()
        }
        (false, ModrinthAuthFlow::SignUp) => {
            crate::accounts::state::get_signup_url()
        }
    }
    .to_owned())
}

#[tracing::instrument(skip(state, presence, code))]
pub async fn authenticate_finish_flow(
    state: &engine::Engine,
    presence: &Presence,
    code: &str,
) -> engine::Result<ModrinthCredentials> {
    let creds = crate::accounts::state::finish_login_flow(
        code,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;

    creds.upsert(state.pool()).await?;
    sqlx::query("INSERT INTO self_hosted_account_origins (user_id,origin) VALUES (?,?) ON CONFLICT(user_id) DO UPDATE SET origin=excluded.origin")
        .bind(&creds.user_id).bind(engine::self_hosted::accounts::origin(&state.sessions)).execute(state.pool()).await?;
    crate::accounts::self_hosted::set_sessions(&state.sessions, Some(&creds));

    if let Err(error) =
        engine::onboarding_checklist::mark_logged_into_modrinth(state).await
    {
        tracing::warn!(
            "Failed to mark Modrinth login in onboarding checklist: {error}"
        );
    }

    reconnect_friends(state, presence).await?;

    Ok(creds)
}

#[tracing::instrument(skip(state, presence))]
pub async fn logout(
    state: &engine::Engine,
    presence: &Presence,
) -> engine::Result<()> {
    sign_out(state, presence).await
}

async fn sign_out(
    state: &engine::Engine,
    presence: &Presence,
) -> engine::Result<()> {
    ModrinthCredentials::deactivate_all(state.pool()).await?;
    crate::accounts::self_hosted::set_sessions(&state.sessions, None);
    presence.friends_socket.disconnect().await?;

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn get_all(
    state: &engine::Engine,
) -> engine::Result<Vec<ModrinthCredentials>> {
    ModrinthCredentials::get_all(state.pool(), &state.sessions).await
}

#[tracing::instrument(skip(state, presence))]
pub async fn set_active(
    state: &engine::Engine,
    presence: &Presence,
    user_id: &str,
) -> engine::Result<()> {
    let users =
        ModrinthCredentials::get_all(state.pool(), &state.sessions).await?;
    let Some(mut creds) =
        users.into_iter().find(|creds| creds.user_id == user_id)
    else {
        bail!("tried to activate nonexistent Modrinth user with ID {user_id}");
    };

    creds.active = true;
    creds.upsert(state.pool()).await?;
    crate::accounts::self_hosted::set_sessions(&state.sessions, Some(&creds));
    reconnect_friends(state, presence).await?;

    Ok(())
}

#[tracing::instrument(skip(state, presence))]
pub async fn remove_user(
    state: &engine::Engine,
    presence: &Presence,
    user_id: &str,
) -> engine::Result<()> {
    if let Some(creds) =
        ModrinthCredentials::get_all(state.pool(), &state.sessions)
            .await?
            .into_iter()
            .find(|creds| creds.user_id == user_id)
    {
        if let Err(error) = crate::accounts::self_hosted::revoke(
            &creds,
            &state.api_semaphore,
            &state.sessions,
        )
        .await
        {
            tracing::warn!("Could not revoke account session: {error}");
        }
    }
    ModrinthCredentials::remove(user_id, state.pool()).await?;

    if ModrinthCredentials::get_active(state.pool())
        .await?
        .is_none()
    {
        crate::accounts::self_hosted::set_sessions(&state.sessions, None);
        presence.friends_socket.disconnect().await?;
    }

    Ok(())
}

async fn reconnect_friends(
    state: &engine::Engine,
    presence: &Presence,
) -> engine::Result<()> {
    if let Err(error) = presence.friends_socket.disconnect().await {
        tracing::warn!("Failed to disconnect friends socket: {error}");
    }
    if let Err(error) = presence
        .friends_socket
        .connect(
            state.pool(),
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

#[tracing::instrument(skip(state, presence))]
pub async fn get_credentials(
    state: &engine::Engine,
    presence: &Presence,
) -> engine::Result<Option<ModrinthCredentials>> {
    let current = ModrinthCredentials::get_and_refresh(
        state.pool(),
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;
    if current.is_none() {
        presence.friends_socket.disconnect().await?;
    }

    crate::accounts::self_hosted::set_sessions(
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
        engine::install_crypto_provider();
        let dir = tempfile::tempdir().unwrap();
        let accounts =
            engine::AccountEndpoints::new("https://accounts.example/", None)
                .unwrap();
        let state = engine::Engine::new(
            crate::data_dir::engine_config(
                dir.path().to_path_buf(),
                engine::EventSink::new(),
                Some(accounts),
            )
            .await
            .unwrap(),
        )
        .await
        .unwrap();
        let presence = Presence::init().unwrap();
        let credentials = ModrinthCredentials {
            session: "session".into(),
            expires: chrono::Utc::now() + chrono::Duration::days(1),
            user_id: "user".into(),
            active: true,
        };
        credentials.upsert(state.pool()).await.unwrap();
        crate::accounts::self_hosted::set_sessions(
            &state.sessions,
            Some(&credentials),
        );

        sign_out(&state, &presence).await.unwrap();

        assert!(
            state
                .sessions
                .modrinth_token("https://accounts.example/v2/user")
                .is_none()
        );
        assert!(state.sessions.backend().is_none());
    }
}
