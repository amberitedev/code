//! Group management via Supabase

use crate::error::{AmberiteError, Result};
use crate::settings::AppSettings;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use std::sync::Arc;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Group {
    pub id: String,
    pub name: String,
    pub owner_id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GroupMember {
    pub user_id: String,
    pub display_name: String,
    pub role: String,
}

fn auth_header(s: &AppSettings) -> Result<String> {
    s.supabase_token
        .as_deref()
        .map(|t| format!("Bearer {}", t))
        .ok_or(AmberiteError::Auth("Not logged in".into()))
}

pub async fn get_groups(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
) -> Result<Vec<Group>> {
    let s = settings.read().await;
    let auth = auth_header(&s)?;
    let url = format!("{}/rest/v1/groups?select=*", supabase_url);
    Ok(client.get(&url).header("Authorization", auth)
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Http(e.to_string()))?)
}

pub async fn create_group(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    name: &str,
) -> Result<Group> {
    let s = settings.read().await;
    let auth = auth_header(&s)?;
    let url = format!("{}/rest/v1/groups", supabase_url);
    let body = serde_json::json!({ "name": name });
    Ok(client.post(&url).header("Authorization", auth).json(&body)
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Http(e.to_string()))?)
}

pub async fn get_group_members(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    group_id: &str,
) -> Result<Vec<GroupMember>> {
    let s = settings.read().await;
    let auth = auth_header(&s)?;
    let url = format!(
        "{}/rest/v1/group_members?group_id=eq.{}&select=*",
        supabase_url, group_id
    );
    Ok(client.get(&url).header("Authorization", auth)
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Http(e.to_string()))?)
}

pub async fn update_member_role(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    group_id: &str,
    user_id: &str,
    role: &str,
) -> Result<()> {
    let s = settings.read().await;
    let auth = auth_header(&s)?;
    let url = format!(
        "{}/rest/v1/group_members?group_id=eq.{}&user_id=eq.{}",
        supabase_url, group_id, user_id
    );
    let body = serde_json::json!({ "role": role });
    client.patch(&url).header("Authorization", auth).json(&body)
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?;
    Ok(())
}

pub async fn create_invite_link(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    group_id: &str,
) -> Result<String> {
    let s = settings.read().await;
    let auth = auth_header(&s)?;
    let url = format!("{}/functions/v1/create-invite", supabase_url);
    let body = serde_json::json!({ "group_id": group_id });
    let resp: serde_json::Value = client.post(&url).header("Authorization", auth).json(&body)
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Http(e.to_string()))?;
    resp["invite_link"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| AmberiteError::Core("No invite_link in response".into()))
}

pub async fn join_via_invite(
    client: &Client,
    settings: &Arc<RwLock<AppSettings>>,
    supabase_url: &str,
    invite_code: &str,
) -> Result<Group> {
    let s = settings.read().await;
    let auth = auth_header(&s)?;
    let url = format!("{}/functions/v1/join-group", supabase_url);
    let body = serde_json::json!({ "invite_code": invite_code });
    Ok(client.post(&url).header("Authorization", auth).json(&body)
        .send().await?.error_for_status()
        .map_err(|e| AmberiteError::Http(e.to_string()))?
        .json().await.map_err(|e| AmberiteError::Http(e.to_string()))?)
}
