use crate::self_hosted::accounts::{AccountEndpoints, may_send_credentials};
use std::sync::{Arc, RwLock};

/// Who this engine calls Modrinth and the backend as. Sign-in sets both; every call reads them.
/// Clones share the same sessions.
#[derive(Clone)]
pub struct Sessions {
    modrinth: Arc<RwLock<Option<ModrinthSession>>>,
    backend: Arc<RwLock<Option<BackendSession>>>,
    pub(crate) accounts: Option<Arc<AccountEndpoints>>,
}

#[derive(Clone)]
pub struct ModrinthSession {
    pub token: String,
}

#[derive(Clone)]
pub struct BackendSession {
    pub base_url: reqwest::Url,
    pub bearer: String,
    pub kind: BackendSessionKind,
}

#[derive(Clone)]
pub enum BackendSessionKind {
    User { user_id: String },
    CoreKey,
}

impl Sessions {
    pub fn new(accounts: Option<AccountEndpoints>) -> Self {
        Self {
            modrinth: Arc::default(),
            backend: Arc::default(),
            accounts: accounts.map(Arc::new),
        }
    }

    pub fn set_modrinth_session(&self, session: Option<ModrinthSession>) {
        *self.modrinth.write().expect("sessions lock poisoned") = session;
    }

    pub fn set_backend_session(&self, session: Option<BackendSession>) {
        *self.backend.write().expect("sessions lock poisoned") = session;
    }

    pub fn backend(&self) -> Option<BackendSession> {
        self.backend.read().expect("sessions lock poisoned").clone()
    }

    /// The Modrinth session token, only for a URL allowed to receive it.
    pub fn modrinth_token(&self, url: &str) -> Option<String> {
        if !may_send_credentials(self, url) {
            return None;
        }
        self.modrinth
            .read()
            .expect("sessions lock poisoned")
            .as_ref()
            .map(|session| session.token.clone())
    }
}
