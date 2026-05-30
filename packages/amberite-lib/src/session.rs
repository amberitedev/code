//! Session token storage backed by the operating system keychain.

use keyring::{Entry, Error as KeyringError};

use crate::error::{AmberiteError, Result};

const SERVICE: &str = "dev.amberite.app";
const SESSION_KEY: &str = "amberite-session-jwt";
const REFRESH_TOKEN_KEY: &str = "amberite-session-refresh-token";

fn session_entry(key: &str) -> Result<Entry> {
	Entry::new(SERVICE, key).map_err(|e| {
		AmberiteError::Auth(format!(
			"failed to open session keychain entry: {e}"
		))
	})
}

fn get_keychain_value(key: &str) -> Result<Option<String>> {
	match session_entry(key)?.get_password() {
		Ok(value) => Ok(Some(value)),
		Err(KeyringError::NoEntry) => Ok(None),
		Err(e) => Err(AmberiteError::Auth(format!(
			"failed to read session token from keychain: {e}"
		))),
	}
}

fn set_keychain_value(key: &str, value: &str) -> Result<()> {
	session_entry(key)?.set_password(value).map_err(|e| {
		AmberiteError::Auth(format!(
			"failed to store session token in keychain: {e}"
		))
	})
}

fn clear_keychain_value(key: &str) -> Result<()> {
	match session_entry(key)?.delete_credential() {
		Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
		Err(e) => Err(AmberiteError::Auth(format!(
			"failed to clear session token from keychain: {e}"
		))),
	}
}

pub async fn get_current_jwt() -> Result<Option<String>> {
	tokio::task::spawn_blocking(|| get_keychain_value(SESSION_KEY))
		.await
		.map_err(|e| {
			AmberiteError::Auth(format!("keychain task panicked: {e}"))
		})?
}

pub async fn set_current_jwt(jwt: String) -> Result<()> {
	tokio::task::spawn_blocking(move || set_keychain_value(SESSION_KEY, &jwt))
		.await
		.map_err(|e| {
			AmberiteError::Auth(format!("keychain task panicked: {e}"))
		})?
}

pub async fn clear_current_jwt() -> Result<()> {
	tokio::task::spawn_blocking(|| clear_keychain_value(SESSION_KEY))
		.await
		.map_err(|e| {
			AmberiteError::Auth(format!("keychain task panicked: {e}"))
		})?
}

pub async fn get_refresh_token() -> Result<Option<String>> {
	tokio::task::spawn_blocking(|| get_keychain_value(REFRESH_TOKEN_KEY))
		.await
		.map_err(|e| {
			AmberiteError::Auth(format!("keychain task panicked: {e}"))
		})?
}

pub async fn set_current_tokens(
	jwt: String,
	refresh_token: String,
) -> Result<()> {
	tokio::task::spawn_blocking(move || {
		set_keychain_value(SESSION_KEY, &jwt)?;
		set_keychain_value(REFRESH_TOKEN_KEY, &refresh_token)
	})
	.await
	.map_err(|e| AmberiteError::Auth(format!("keychain task panicked: {e}")))?
}

pub async fn clear_current_session() -> Result<()> {
	tokio::task::spawn_blocking(|| {
		clear_keychain_value(SESSION_KEY)?;
		clear_keychain_value(REFRESH_TOKEN_KEY)
	})
	.await
	.map_err(|e| AmberiteError::Auth(format!("keychain task panicked: {e}")))?
}
