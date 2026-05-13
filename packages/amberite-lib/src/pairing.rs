//! Core pairing — localhost auto-pair and remote code pairing with machine-account provisioning.

use crate::error::{AmberiteError, Result};
use crate::settings::AppSettings;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Serialize, Deserialize)]
struct SetupRequest {
	pairing_code: String,
	owner_user_id: String,
	supabase_email: String,
	supabase_password: String,
	supabase_url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct SetupResponse {
	paired: bool,
	message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PairResult {
	pub core_url: String,
	pub paired: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProvisioningRequest {
	core_id: String,
	owner_user_id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct ProvisioningResponse {
	core_id: String,
	email: String,
	password: String,
	user_id: String,
}

/// Call the Supabase Edge Function to provision a machine account for Core.
async fn provision_machine_account(
	client: &Client,
	supabase_url: &str,
	core_id: &str,
	owner_user_id: &str,
) -> Result<ProvisioningResponse> {
	let url = format!("{}/functions/v1/provision-core-machine-account", supabase_url.trim_end_matches('/'));
	let resp = client
		.post(&url)
		.header("Content-Type", "application/json")
		.json(&ProvisioningRequest {
			core_id: core_id.to_string(),
			owner_user_id: owner_user_id.to_string(),
		})
		.send()
		.await?;

	if !resp.status().is_success() {
		let msg = resp.text().await.unwrap_or_default();
		return Err(AmberiteError::Core(format!("Machine account provisioning failed: {msg}")));
	}

	let result: ProvisioningResponse = resp.json().await?;
	Ok(result)
}

/// POST to Core `/setup` with a pairing code and machine-account credentials.
/// On success, updates settings to mark the app as paired and saves the Core URL.
pub async fn pair_with_code(
	client: &Client,
	settings: &Arc<RwLock<AppSettings>>,
	relay_url: &str,
	code: &str,
) -> Result<PairResult> {
	let s = settings.read().await;
	let user_id = s.supabase_user_id.clone().unwrap_or_default();
	let supabase_url = s.supabase_url.clone().unwrap_or_default();
	drop(s);

	// Generate a stable core_id from the relay_url hostname for V1
	let core_id = relay_url
		.trim_start_matches("http://")
		.trim_start_matches("https://")
		.split(':')
		.next()
		.unwrap_or("unknown")
		.to_string();

	// Provision machine account via Edge Function
	let provisioned = provision_machine_account(client, &supabase_url, &core_id, &user_id).await?;

	let url = format!("{}/setup", relay_url.trim_end_matches('/'));
	let resp = client
		.post(&url)
		.json(&SetupRequest {
			pairing_code: code.to_string(),
			owner_user_id: user_id,
			supabase_email: provisioned.email,
			supabase_password: provisioned.password,
			supabase_url: supabase_url.clone(),
		})
		.send()
		.await?;

	if !resp.status().is_success() {
		let msg = resp.text().await.unwrap_or_default();
		return Err(AmberiteError::Core(format!("Pairing failed: {msg}")));
	}

	let result: SetupResponse = resp.json().await?;
	if !result.paired {
		return Err(AmberiteError::Core(format!(
			"Core rejected pairing: {}",
			result.message.unwrap_or_default()
		)));
	}

	{
		let mut s = settings.write().await;
		s.core_url = Some(relay_url.to_string());
		s.save().await?;
	}

	Ok(PairResult { core_url: relay_url.to_string(), paired: true })
}

/// Auto-pair with a locally running Core by reading its pairing code from a
/// well-known file in the Core data directory.
pub async fn pair_local_auto(
	client: &Client,
	settings: &Arc<RwLock<AppSettings>>,
) -> Result<PairResult> {
	let code_file = dirs::data_dir()
		.ok_or_else(|| AmberiteError::Core("Cannot find data directory".to_string()))?
		.join("amberite-core")
		.join("data")
		.join(".pairing_code");

	let mut attempts = 0u8;
	let code = loop {
		if code_file.exists() {
			break tokio::fs::read_to_string(&code_file).await?;
		}
		if attempts >= 20 {
			return Err(AmberiteError::Core(
				"Timed out waiting for Core pairing code".to_string(),
			));
		}
		tokio::time::sleep(std::time::Duration::from_millis(500)).await;
		attempts += 1;
	};

	let code = code.trim().to_string();
	pair_with_code(client, settings, "http://localhost:7000", &code).await
}
