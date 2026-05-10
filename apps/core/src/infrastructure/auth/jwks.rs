use std::time::{Duration, Instant};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use serde::{Deserialize, Serialize};
use tracing::debug;

const CACHE_TTL: Duration = Duration::from_secs(3600);

/// Cached JWKS data for Supabase RS256 validation.
pub struct JwksCache {
    inner: tokio::sync::RwLock<Option<CacheEntry>>,
    http: reqwest::Client,
}

struct CacheEntry {
    keys: Vec<JwkEntry>,
    fetched_at: Instant,
}

struct JwkEntry {
    kid: Option<String>,
    key: DecodingKey,
}

#[derive(Debug, Deserialize)]
struct JwksDoc {
    keys: Vec<JwkRaw>,
}

#[derive(Debug, Deserialize)]
struct JwkRaw {
    kid: Option<String>,
    kty: String,
    n: Option<String>,
    e: Option<String>,
}

/// Claims extracted from a valid Supabase JWT.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,
    pub role: Option<String>,
    pub exp: u64,
}

impl JwksCache {
    pub fn new(http: reqwest::Client) -> Self {
        Self { inner: tokio::sync::RwLock::new(None), http }
    }

    /// Validate a JWT against the JWKS from `jwks_url`.
    pub async fn validate(&self, token: &str, jwks_url: &str) -> Result<Claims, AuthError> {
        self.refresh_if_stale(jwks_url).await?;
        let guard = self.inner.read().await;
        let Some(ref entry) = *guard else {
            return Err(AuthError::NoKeys);
        };

        let header = decode_header(token).map_err(|_| AuthError::InvalidToken)?;
        let token_kid = header.kid.as_deref();

        for jwk in &entry.keys {
            // If both have kid, they must match. If no kid, try all keys.
            let kid_matches = match (token_kid, jwk.kid.as_deref()) {
                (Some(a), Some(b)) => a == b,
                _ => true,
            };
            if !kid_matches {
                continue;
            }
            let mut validation = Validation::new(Algorithm::RS256);
            validation.validate_exp = true;
            if let Ok(data) = decode::<Claims>(token, &jwk.key, &validation) {
                return Ok(data.claims);
            }
        }
        Err(AuthError::InvalidToken)
    }

    async fn refresh_if_stale(&self, jwks_url: &str) -> Result<(), AuthError> {
        {
            let guard = self.inner.read().await;
            if let Some(ref e) = *guard {
                if e.fetched_at.elapsed() < CACHE_TTL {
                    return Ok(());
                }
            }
        }
        debug!("Refreshing JWKS from {jwks_url}");
        let doc: JwksDoc = self.http.get(jwks_url).send().await
            .map_err(|e| AuthError::Fetch(e.to_string()))?
            .json().await
            .map_err(|e| AuthError::Fetch(e.to_string()))?;

        let mut keys = Vec::new();
        for raw in doc.keys {
            if raw.kty != "RSA" { continue; }
            if let (Some(n), Some(e)) = (raw.n, raw.e) {
                let n_bytes = URL_SAFE_NO_PAD.decode(&n).map_err(|_| AuthError::BadKey)?;
                let e_bytes = URL_SAFE_NO_PAD.decode(&e).map_err(|_| AuthError::BadKey)?;
                let key = DecodingKey::from_rsa_raw_components(&n_bytes, &e_bytes);
                keys.push(JwkEntry { kid: raw.kid, key });
            }
        }
        *self.inner.write().await = Some(CacheEntry { keys, fetched_at: Instant::now() });
        Ok(())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    #[error("invalid token")]
    InvalidToken,
    #[error("no JWKS keys available")]
    NoKeys,
    #[error("failed to fetch JWKS: {0}")]
    Fetch(String),
    #[error("bad key encoding")]
    BadKey,
    #[error("core not paired with Supabase")]
    NotPaired,
}
