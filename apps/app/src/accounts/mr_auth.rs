use crate::accounts::state::ModrinthCredentials;
use crate::presence::Presence;
use crate::settings::{FeatureFlag, Settings};
use crate::state::Desktop;
use anyhow::bail;
use serde::Deserialize;

const LOCALHOST_LOGIN_URL: &str = "http://localhost:3000/auth/sign-in";
const LOCALHOST_SIGNUP_URL: &str = "http://localhost:3000/auth/sign-up";

#[derive(Clone, Copy, Debug, Deserialize)]
#[cfg_attr(feature = "export-ts", derive(ts_rs::TS))]
#[serde(rename_all = "kebab-case")]
pub enum ModrinthAuthFlow {
    SignIn,
    SignUp,
}

#[tracing::instrument(skip(state))]
pub async fn authenticate_begin_flow(
    state: &Desktop,
    flow: ModrinthAuthFlow,
) -> engine::Result<String> {
    if let Some(url) = engine::self_hosted::accounts::login_url(
        &state.sessions,
        matches!(flow, ModrinthAuthFlow::SignUp),
    ) {
        return Ok(url);
    }
    let settings = Settings::get(state.db()).await?;
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
    state: &Desktop,
    presence: &Presence,
    code: &str,
) -> engine::Result<ModrinthCredentials> {
    let creds = crate::accounts::state::finish_login_flow(
        code,
        &state.api_semaphore,
        &state.sessions,
    )
    .await?;

    creds.upsert(state.db()).await?;
    sqlx::query("INSERT INTO self_hosted_account_origins (user_id,origin) VALUES (?,?) ON CONFLICT(user_id) DO UPDATE SET origin=excluded.origin")
        .bind(&creds.user_id).bind(engine::self_hosted::accounts::origin(&state.sessions)).execute(state.db()).await?;
    crate::accounts::self_hosted::set_sessions(&state.sessions, Some(&creds));

    if let Err(error) =
        crate::onboarding::mark_logged_into_modrinth(state).await
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
    state: &Desktop,
    presence: &Presence,
) -> engine::Result<()> {
    sign_out(state, presence).await
}

async fn sign_out(state: &Desktop, presence: &Presence) -> engine::Result<()> {
    ModrinthCredentials::deactivate_all(state.db()).await?;
    crate::accounts::self_hosted::set_sessions(&state.sessions, None);
    presence.friends_socket.disconnect().await?;

    Ok(())
}

#[tracing::instrument(skip(state))]
pub async fn get_all(
    state: &Desktop,
) -> engine::Result<Vec<ModrinthCredentials>> {
    ModrinthCredentials::get_all(state.db(), &state.sessions).await
}

#[tracing::instrument(skip(state, presence))]
pub async fn set_active(
    state: &Desktop,
    presence: &Presence,
    user_id: &str,
) -> engine::Result<()> {
    let users =
        ModrinthCredentials::get_all(state.db(), &state.sessions).await?;
    let Some(mut creds) =
        users.into_iter().find(|creds| creds.user_id == user_id)
    else {
        bail!("tried to activate nonexistent Modrinth user with ID {user_id}");
    };

    creds.active = true;
    creds.upsert(state.db()).await?;
    crate::accounts::self_hosted::set_sessions(&state.sessions, Some(&creds));
    reconnect_friends(state, presence).await?;

    Ok(())
}

#[tracing::instrument(skip(state, presence))]
pub async fn remove_user(
    state: &Desktop,
    presence: &Presence,
    user_id: &str,
) -> engine::Result<()> {
    if let Some(creds) =
        ModrinthCredentials::get_all(state.db(), &state.sessions)
            .await?
            .into_iter()
            .find(|creds| creds.user_id == user_id)
        && let Err(error) =
            crate::accounts::self_hosted::revoke(&creds, &state.sessions).await
    {
        tracing::warn!("Could not revoke account session: {error}");
    }
    ModrinthCredentials::remove(user_id, state.db()).await?;

    if ModrinthCredentials::get_active(state.db()).await?.is_none() {
        crate::accounts::self_hosted::set_sessions(&state.sessions, None);
        presence.friends_socket.disconnect().await?;
    }

    Ok(())
}

async fn reconnect_friends(
    state: &Desktop,
    presence: &Presence,
) -> engine::Result<()> {
    if let Err(error) = presence.friends_socket.disconnect().await {
        tracing::warn!("Failed to disconnect friends socket: {error}");
    }
    if let Err(error) = presence
        .friends_socket
        .connect(
            state.db(),
            &state.api_semaphore,
            &state.sessions,
            &state.process_manager,
            state.app_events(),
        )
        .await
    {
        tracing::warn!("Failed to reconnect friends socket: {error}");
    }

    Ok(())
}

#[tracing::instrument(skip(state, presence))]
pub async fn get_credentials(
    state: &Desktop,
    presence: &Presence,
) -> engine::Result<Option<ModrinthCredentials>> {
    let current = ModrinthCredentials::get_and_refresh(
        state.db(),
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
        let state = crate::data_dir::tests::open(dir.path(), Some(accounts))
            .await
            .unwrap();
        let presence = Presence::init().unwrap();
        let credentials = ModrinthCredentials {
            session: "session".into(),
            expires: chrono::Utc::now() + chrono::Duration::days(1),
            user_id: "user".into(),
            active: true,
        };
        credentials.upsert(state.db()).await.unwrap();
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
