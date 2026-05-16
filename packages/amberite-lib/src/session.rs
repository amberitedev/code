//! Session token storage backed by the operating system keychain.

use keyring::{Entry, Error as KeyringError};

use crate::error::{AmberiteError, Result};

const SERVICE: &str = "dev.amberite.app";
const SESSION_KEY: &str = "amberite-session-jwt";

fn session_entry() -> Result<Entry> {
	Entry::new(SERVICE, SESSION_KEY)
		.map_err(|e| AmberiteError::Auth(format!("failed to open session keychain entry: {e}")))
}

pub fn get_current_jwt() -> Result<Option<String>> {
	match session_entry()?.get_password() {
		Ok(jwt) => Ok(Some(jwt)),
		Err(KeyringError::NoEntry) => Ok(None),
		Err(e) => Err(AmberiteError::Auth(format!(
			"failed to read session token from keychain: {e}"
		))),
	}
}

pub fn set_current_jwt(jwt: String) -> Result<()> {
	session_entry()?.set_password(&jwt).map_err(|e| {
		AmberiteError::Auth(format!("failed to store session token in keychain: {e}"))
	})
}

pub fn clear_current_jwt() -> Result<()> {
	match session_entry()?.delete_credential() {
		Ok(()) | Err(KeyringError::NoEntry) => Ok(()),
		Err(e) => Err(AmberiteError::Auth(format!(
			"failed to clear session token from keychain: {e}"
		))),
	}
}
