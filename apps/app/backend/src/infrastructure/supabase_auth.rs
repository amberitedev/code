use jsonwebtoken::{
    decode, decode_header, Algorithm, DecodingKey, Validation,
    JwkSetForEc, jwk_from_ec,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use tokio::sync::RwLock;
use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SupabaseClaims {
    pub sub: String,
    pub email: Option<String>,
    pub role: Option<String>,
    pub aud: Option<String>,
    pub iss: Option<String>,
    pub exp: Option<usize>,
    pub iat: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JwkKey {
    kty: String,
    use_: Option<String>,
    kid: Option<String>,
    alg: Option<String>,
    #[serde(rename = "crv")]
    crv: Option<String>,
    x: Option<String>,
    y: Option<String>,
    n: Option<String>,
    e: Option<String>,
}

#[derive(Error, Debug)]
pub enum JwtError {
    #[error("Missing token")]
    MissingToken,
    #[error("Invalid token format")]
    InvalidFormat,
    #[error("Token validation failed: {0}")]
    ValidationFailed(String),
    #[error("Token expired")]
    Expired,
    #[error("Invalid audience: {0}")]
    InvalidAudience(String),
    #[error("Invalid issuer")]
    InvalidIssuer,
    #[error("JWKS fetch failed: {0}")]
    JwksFetchFailed(String),
    #[error("No matching key found")]
    NoMatchingKey,
    #[error("Key not found for kid: {0}")]
    KeyNotFound(String),
}

pub struct JwksCache {
    keys: HashMap<String, jsonwebtoken::jwk::Jwk>,
    fetched_at: u64,
    ttl_seconds: u64,
}

impl JwksCache {
    pub fn new(ttl_seconds: u64) -> Self {
        Self {
            keys: HashMap::new(),
            fetched_at: 0,
            ttl_seconds,
        }
    }

    pub fn is_expired(&self) -> bool {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        now - self.fetched_at > self.ttl_seconds
    }

    pub fn update(&mut self, keys: HashMap<String, jsonwebtoken::jwk::Jwk>) {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs();
        self.fetched_at = now;
        self.keys = keys;
    }

    pub fn get_key(&self, kid: &str) -> Option<&jsonwebtoken::jwk::Jwk> {
        self.keys.get(kid)
    }
}

pub struct SupabaseJwksValidator {
    jwks_cache: Arc<RwLock<JwksCache>>,
    jwks_url: String,
    expected_issuer: String,
    required_audience: String,
}

impl SupabaseJwksValidator {
    pub fn new(supabase_url: &str, ttl_seconds: u64) -> Self {
        let jwks_url = format!("{}/auth/v1/.well-known/jwks.json", supabase_url.trim_end_matches('/'));
        let expected_issuer = format!("{}/auth/v1", supabase_url.trim_end_matches('/'));
        
        Self {
            jwks_cache: Arc::new(RwLock::new(JwksCache::new(ttl_seconds))),
            jwks_url,
            expected_issuer,
            required_audience: "authenticated".to_string(),
        }
    }

    pub async fn fetch_jwks(&self) -> Result<(), JwtError> {
        let response = reqwest::get(&self.jwks_url)
            .await
            .map_err(|e| JwtError::JwksFetchFailed(e.to_string()))?;
        
        let jwks: JwksResponse = response
            .json()
            .await
            .map_err(|e| JwtError::JwksFetchFailed(e.to_string()))?;
        
        let mut keys = HashMap::new();
        
        for key in jwks.keys {
            if let (Some(kid), (Some(x), Some(y))) = (&key.kid, &key.x, &key.y) {
                let jwk = jsonwebtoken::jwk::Jwk::new_ec_key(
                    jsonwebtoken::jwk::EcCurve::P256,
                    x,
                    y,
                ).map_err(|e| JwtError::JwksFetchFailed(e.to_string()))?;
                
                keys.insert(kid.clone(), jwk);
            }
        }
        
        let mut cache = self.jwks_cache.write().await;
        cache.update(keys);
        
        Ok(())
    }

    pub async fn get_or_fetch_jwks(&self) -> Result<Arc<RwLock<JwksCache>>, JwtError> {
        let cache = self.jwks_cache.read().await;
        
        if cache.keys.is_empty() || cache.is_expired() {
            drop(cache);
            self.fetch_jwks().await?;
            return Ok(self.jwks_cache.clone());
        }
        
        Ok(self.jwks_cache.clone())
    }

    pub async fn validate(&self, token: &str) -> Result<SupabaseClaims, JwtError> {
        let token = token.strip_prefix("Bearer ").unwrap_or(token);
        
        let header = decode_header(token)
            .map_err(|_| JwtError::InvalidFormat)?;
        
        let kid = header.kid.ok_or(JwtError::InvalidFormat)?;
        
        let cache = self.get_or_fetch_jwks().await?;
        let cache = cache.read().await;
        
        let jwk = cache.get_key(&kid)
            .ok_or(JwtError::KeyNotFound(kid))?;
        
        let decoding_key = DecodingKey::from_jwk(jwk)
            .map_err(|e| JwtError::ValidationFailed(e.to_string()))?;
        
        let mut validation = Validation::new(Algorithm::ES256);
        validation.validate_exp = true;
        validation.set_issuer(&[&self.expected_issuer]);
        validation.set_audience(&[&self.required_audience]);
        
        let decoded = decode::<SupabaseClaims>(token, &decoding_key, &validation)
            .map_err(|e| {
                if e.to_string().contains("ExpiredSignature") {
                    JwtError::Expired
                } else if e.to_string().contains("InvalidAudience") {
                    JwtError::InvalidAudience(self.required_audience.clone())
                } else if e.to_string().contains("InvalidIssuer") {
                    JwtError::InvalidIssuer
                } else {
                    JwtError::ValidationFailed(e.to_string())
                }
            })?;
        
        Ok(decoded.claims)
    }

    pub async fn validate_with_refresh(&self, token: &str) -> Result<SupabaseClaims, JwtError> {
        let result = self.validate(token).await;
        
        if let Err(JwtError::KeyNotFound(ref kid)) = result {
            self.fetch_jwks().await?;
            return self.validate(token).await;
        }
        
        result
    }
}

// Sync wrapper for Axum extractor compatibility
pub struct SupabaseJwtValidator {
    inner: std::sync::Arc<tokio::sync::RwLock<Option<SupabaseJwksValidator>>,
}

impl SupabaseJwtValidator {
    pub fn new(supabase_url: &str) -> Self {
        Self {
            inner: std::sync::Arc::new(tokio::sync::RwLock::new(Some(
                SupabaseJwksValidator::new(supabase_url, 3600)
            ))),
        }
    }

    pub fn validate(&self, token: &str) -> impl std::future::Future<Output = Result<SupabaseClaims, JwtError> + '_ {
        async move {
            let guard = self.inner.read().await;
            if let Some(validator) = guard.as_ref() {
                validator.validate_with_refresh(token).await
            } else {
                Err(JwtError::ValidationFailed("Validator not initialized".to_string()))
            }
        }
    }
}