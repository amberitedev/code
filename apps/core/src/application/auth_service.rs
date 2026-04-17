//! Auth service - Login, registration, token generation with PASETO.

use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;
use argon2::{Argon2, PasswordHash, PasswordVerifier, PasswordHasher};
use argon2::password_hash::SaltString;
use rand::rngs::OsRng;
use crate::domain::auth::{AuthTokenPayload, UserId, User, Role, UserPermission};
use crate::domain::ports::UserRepository;

/// Auth error type.
#[derive(Error, Debug)]
pub enum AuthError {
    #[error("Invalid credentials")]
    InvalidCredentials,
    #[error("User not found")]
    UserNotFound,
    #[error("Token generation failed")]
    TokenGeneration,
    #[error("Password hashing failed")]
    PasswordHashing,
}

/// Authentication service.
pub struct AuthService {
    user_repo: Arc<dyn UserRepository>,
    paseto_key: [u8; 32],
    token_expiry_seconds: i64,
}

impl AuthService {
    pub fn new(user_repo: Arc<dyn UserRepository>, _paseto_key: [u8; 32]) -> Self {
        AuthService {
            user_repo,
            paseto_key: _paseto_key,
            token_expiry_seconds: 86400,
        }
    }

    pub async fn authenticate(&self, username: &str, password: &str) -> Result<String, AuthError> {
        let user = self.user_repo
            .get_user_by_username(username)
            .await
            .map_err(|_| AuthError::UserNotFound)?;

        let hash = PasswordHash::new(&user.hashed_password)
            .map_err(|_| AuthError::InvalidCredentials)?;

        Argon2::default()
            .verify_password(password.as_bytes(), &hash)
            .map_err(|_| AuthError::InvalidCredentials)?;

        let payload = AuthTokenPayload::new(
            user.id,
            chrono::Utc::now().timestamp() + self.token_expiry_seconds,
        );

        // For now, just return a simple token (we can implement proper PASETO later)
        let token = format!("{}:{}:{}", payload.user_id, payload.expires_at, Uuid::new_v4());
        
        Ok(token)
    }

    pub async fn register(&self, username: &str, password: &str) -> Result<User, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let argon2 = Argon2::default();
        let hash = argon2.hash_password(password.as_bytes(), &salt)
            .map_err(|_| AuthError::PasswordHashing)?
            .to_string();

        let user = User {
            id: UserId::new(Uuid::new_v4()),
            username: username.to_string(),
            hashed_password: hash,
            role: Role::Standard(UserPermission::default()),
        };

        self.user_repo.create_user(&user).await
            .map_err(|_| AuthError::UserNotFound)?;

        Ok(user)
    }

    pub fn validate_token(&self, token: &str) -> Result<UserId, AuthError> {
        // For now, just parse the simple token (we can implement proper PASETO later)
        let parts: Vec<&str> = token.split(':').collect();
        if parts.len() != 3 {
            return Err(AuthError::InvalidCredentials);
        }

        let user_id_str = parts[0];
        let expires_at_str = parts[1];

        // Parse user ID
        let user_id = UserId::new(Uuid::parse_str(user_id_str).map_err(|_| AuthError::InvalidCredentials)?);

        // Parse expiration
        let expires_at = expires_at_str.parse::<i64>().map_err(|_| AuthError::InvalidCredentials)?;
        
        if expires_at < chrono::Utc::now().timestamp() {
            return Err(AuthError::InvalidCredentials);
        }

        Ok(user_id)
    }
}
