//! Supabase authentication via Microsoft identity token

use crate::error::{AmberiteError, Result};
use crate::settings::AppSettings;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;

const SUPABASE_AUTH_FUNCTION: &str = "microsoft-auth";

/// Response from the `microsoft-auth` Edge Function
#[derive(Debug, Deserialize)]
struct MicrosoftAuthResponse {
    access_token: String,
    user_id: String,
    display_name: String,
}

/// Authenticated user info returned to the frontend
#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AuthUser {
    pub user_id: String,
    pub display_name: String,
}

/// Exchange a Microsoft identity token for a Supabase JWT.
/// Calls the `microsoft-auth` Edge Function, then persists the token.
pub async fn login_microsoft(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    microsoft_id_token: &str,
) -> Result<AuthUser> {
    let url = format!("{}/functions/v1/{}", supabase_url, SUPABASE_AUTH_FUNCTION);

    let body = serde_json::json!({ "id_token": microsoft_id_token });

    let resp = client
        .post(&url)
        .json(&body)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Auth(e.to_string()))?;

    let auth: MicrosoftAuthResponse = resp
        .json()
        .await
        .map_err(|e| AmberiteError::Auth(e.to_string()))?;

    let user = AuthUser {
        user_id: auth.user_id.clone(),
        display_name: auth.display_name.clone(),
    };

    // Persist token to settings
    {
        let mut s = settings.write().await;
        s.supabase_token = Some(auth.access_token);
        s.supabase_user_id = Some(auth.user_id);
        s.display_name = Some(auth.display_name);
        s.save().await?;
    }

    Ok(user)
}

/// Clear the stored Supabase token and user info
pub async fn logout(settings: &Arc<RwLock<AppSettings>>) -> Result<()> {
    let mut s = settings.write().await;
    s.supabase_token = None;
    s.supabase_user_id = None;
    s.display_name = None;
    s.save().await
}

/// Return the stored Supabase JWT synchronously if available (for streaming use).
/// Returns empty string if not authenticated.
pub fn get_jwt_sync(settings: &AppSettings) -> String {
    settings.supabase_token.clone().unwrap_or_default()
}

/// Return the currently logged-in user, if any
pub async fn get_current_user(settings: &Arc<RwLock<AppSettings>>) -> Option<AuthUser> {
    let s = settings.read().await;
    match (&s.supabase_user_id, &s.display_name) {
        (Some(id), Some(name)) => Some(AuthUser {
            user_id: id.clone(),
            display_name: name.clone(),
        }),
        _ => None,
    }
}
