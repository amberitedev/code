//! Friends management via Supabase

use crate::error::{AmberiteError, Result};
use crate::settings::AppSettings;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Friend {
    pub user_id: String,
    pub display_name: String,
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct UserProfile {
    pub user_id: String,
    pub display_name: String,
}

async fn get_jwt(settings: &Arc<RwLock<AppSettings>>) -> Result<String> {
    settings.read().await.supabase_token.clone()
        .ok_or(AmberiteError::Auth("Not logged in".into()))
}

pub async fn get_friends(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
) -> Result<Vec<Friend>> {
    let jwt = get_jwt(settings).await?;
    let url = format!("{}/rest/v1/friends?select=*", supabase_url);
    Ok(client.get(&url).bearer_auth(&jwt).send().await?
        .error_for_status().map_err(|e| AmberiteError::Http(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Http(e.to_string()))?)
}

pub async fn add_friend(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    target_user_id: &str,
) -> Result<()> {
    let jwt = get_jwt(settings).await?;
    let url = format!("{}/rest/v1/friend_requests", supabase_url);
    client.post(&url).bearer_auth(&jwt)
        .json(&serde_json::json!({ "to_user_id": target_user_id }))
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?;
    Ok(())
}

pub async fn remove_friend(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    target_user_id: &str,
) -> Result<()> {
    let jwt = get_jwt(settings).await?;
    let url = format!("{}/rest/v1/friends?user_id=eq.{}", supabase_url, target_user_id);
    client.delete(&url).bearer_auth(&jwt).send().await?
        .error_for_status().map_err(|e| AmberiteError::Http(e.to_string()))?;
    Ok(())
}

pub async fn block_user(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    target_user_id: &str,
) -> Result<()> {
    let jwt = get_jwt(settings).await?;
    let url = format!("{}/rest/v1/blocked_users", supabase_url);
    client.post(&url).bearer_auth(&jwt)
        .json(&serde_json::json!({ "blocked_user_id": target_user_id }))
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?;
    Ok(())
}

pub async fn lookup_user(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    display_name: &str,
) -> Result<UserProfile> {
    let jwt = get_jwt(settings).await?;
    let url = format!("{}/rest/v1/profiles?display_name=eq.{}&select=*", supabase_url, display_name);
    let mut profiles: Vec<UserProfile> = client.get(&url).bearer_auth(&jwt)
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Http(e.to_string()))?;
    profiles.pop().ok_or_else(|| AmberiteError::NotFound(format!("User '{display_name}' not found")))
}
