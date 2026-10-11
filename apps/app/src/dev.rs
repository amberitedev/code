use serde::{Deserialize, Serialize};
use std::env;
use std::sync::OnceLock;

#[derive(Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DevAppConfig {
    pub credential_namespace: String,
    pub auth_mode: String,
    pub branch: String,
    pub title: String,
    pub data_dir: String,
    pub backend_url: Option<String>,
    pub username: Option<String>,
}

static CONFIG: OnceLock<Option<DevAppConfig>> = OnceLock::new();

pub fn prepare() {
    #[cfg(debug_assertions)]
    let config = parse_config();
    #[cfg(not(debug_assertions))]
    let config = None;
    let backend_url = config
        .as_ref()
        .and_then(|config| config.backend_url.clone())
        .or_else(|| env::var("ACCOUNT_API_URL").ok());
    if let Some(backend_url) = backend_url {
        theseus::self_hosted::accounts::configure(
            &backend_url,
            env::var("ACCOUNT_WEB_URL").ok().as_deref(),
        )
        .expect("valid account service configuration");
    }
    let _ = CONFIG.set(config);
}

pub fn config() -> Option<&'static DevAppConfig> {
    CONFIG.get().and_then(Option::as_ref)
}

/// Sign a dev App in to its scenario account, so every App the dev runner starts is signed in
/// without going through the website. Does nothing when an account is already active. The backend
/// opens the session only for the runner's secret, which arrives in the environment.
#[cfg(debug_assertions)]
pub async fn sign_in() {
    let Some(config) = config().filter(|config| config.auth_mode == "dev")
    else {
        return;
    };
    let (Some(username), Some(backend_url), Ok(secret)) = (
        &config.username,
        &config.backend_url,
        env::var("AMBERITE_LOCAL_DEV_SECRET"),
    ) else {
        return;
    };
    if let Err(error) = open_session(username, backend_url, &secret).await {
        tracing::warn!("Could not sign in as {username}: {error}");
    }
}

#[cfg(debug_assertions)]
async fn open_session(
    username: &str,
    backend_url: &str,
    secret: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if theseus::mr_auth::get_credentials().await?.is_some() {
        return Ok(());
    }
    #[derive(Deserialize)]
    struct Session {
        session: String,
    }
    let response = tauri_plugin_http::reqwest::Client::new()
        .post(format!(
            "{}/_dev/session",
            backend_url.trim_end_matches('/')
        ))
        .header("Content-Type", "application/json")
        .header("X-Local-Dev-Secret", secret)
        .body(serde_json::json!({ "username": username }).to_string())
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;
    let session: Session = serde_json::from_str(&response)?;
    theseus::mr_auth::authenticate_finish_flow(&session.session).await?;
    Ok(())
}

#[cfg(debug_assertions)]
fn parse_config() -> Option<DevAppConfig> {
    let mut args = env::args();
    while let Some(arg) = args.next() {
        if arg != "--amberite-dev-config" {
            continue;
        }
        let raw = args
            .next()
            .unwrap_or_else(|| panic!("--amberite-dev-config requires JSON"));
        let config: DevAppConfig =
            serde_json::from_str(&raw).unwrap_or_else(|error| {
                panic!("invalid Amberite dev config: {error}")
            });
        assert!(
            !config.branch.trim().is_empty(),
            "dev branch cannot be empty"
        );
        assert!(
            !config.credential_namespace.trim().is_empty(),
            "dev credential namespace cannot be empty"
        );
        assert!(
            !config.data_dir.trim().is_empty(),
            "dev data directory cannot be empty"
        );
        unsafe {
            env::set_var("THESEUS_CONFIG_DIR", &config.data_dir);
            env::set_var(
                "WEBVIEW2_USER_DATA_FOLDER",
                format!("{}/webview2", config.data_dir),
            );
        }
        return Some(config);
    }
    None
}
