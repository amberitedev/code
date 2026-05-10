//! Supabase REST client — authenticated requests with stored JWT

use crate::error::{AmberiteError, Result};
use crate::settings::AppSettings;
use reqwest::{Client, RequestBuilder};
use tokio::sync::RwLock;
use std::sync::Arc;

/// Build an authenticated request to a Supabase REST or Edge Function endpoint.
/// Injects the `Authorization: Bearer <token>` header from stored settings.
pub async fn authed_request(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    path: &str,
) -> Result<RequestBuilder> {
    let token = {
        let s = settings.read().await;
        s.supabase_token
            .clone()
            .ok_or(AmberiteError::Auth("Not logged in".into()))?
    };

    let url = format!("{}{}", supabase_url, path);
    Ok(client
        .get(url)
        .header("Authorization", format!("Bearer {}", token))
        .header("Content-Type", "application/json"))
}

/// POST to a Supabase Edge Function with auth
pub async fn post_function<B: serde::Serialize>(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    function_name: &str,
    body: &B,
) -> Result<reqwest::Response> {
    let token = {
        let s = settings.read().await;
        s.supabase_token
            .clone()
            .ok_or(AmberiteError::Auth("Not logged in".into()))?
    };

    let url = format!("{}/functions/v1/{}", supabase_url, function_name);
    client
        .post(&url)
        .header("Authorization", format!("Bearer {}", token))
        .json(body)
        .send()
        .await?
        .error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))
}
