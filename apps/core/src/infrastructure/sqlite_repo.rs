//! SQLite repository - Implements InstanceRepository and UserRepository.

use sqlx::{Pool, Sqlite, Row};
use uuid::Uuid;
use crate::domain::instances::{InstanceId, GameInstance, Stopped, DomainError};
use crate::domain::ports::{InstanceRepository, UserRepository};
use crate::domain::auth::{UserId, User, Role};

/// SQLite repository.
pub struct SqliteRepo {
    pool: Pool<Sqlite>,
}

impl SqliteRepo {
    pub async fn new(pool: Pool<Sqlite>) -> Self {
        SqliteRepo { pool }
    }
}

#[async_trait::async_trait]
impl InstanceRepository for SqliteRepo {
    async fn get_instance(&self, id: InstanceId) -> Result<GameInstance<Stopped>, DomainError> {
        let row = sqlx::query("SELECT id, name FROM instances WHERE id = ?")
            .bind(id.0.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(GameInstance {
            id,
            name: row.get::<String, _>("name"),
            state: Stopped,
        })
    }

    async fn save_instance(&self, instance: &GameInstance<Stopped>) -> Result<(), DomainError> {
        sqlx::query("INSERT OR REPLACE INTO instances (id, name) VALUES (?, ?)")
            .bind(instance.id.0.to_string())
            .bind(&instance.name)
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;
        Ok(())
    }

    async fn list_instances(&self) -> Result<Vec<GameInstance<Stopped>>, DomainError> {
        let rows = sqlx::query("SELECT id, name FROM instances")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| GameInstance {
            id: InstanceId(Uuid::parse_str(&r.get::<String, _>("id")).unwrap()),
            name: r.get::<String, _>("name"),
            state: Stopped,
        }).collect())
    }

    async fn delete_instance(&self, id: InstanceId) -> Result<(), DomainError> {
        sqlx::query("DELETE FROM instances WHERE id = ?")
            .bind(id.0.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl UserRepository for SqliteRepo {
    async fn get_user(&self, id: UserId) -> Result<User, DomainError> {
        let row = sqlx::query("SELECT id, username, hashed_password, role FROM users WHERE id = ?")
            .bind(id.0.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(User {
            id,
            username: row.get::<String, _>("username"),
            hashed_password: row.get::<String, _>("hashed_password"),
            role: Role::Admin,
        })
    }

    async fn get_user_by_username(&self, username: &str) -> Result<User, DomainError> {
        let row = sqlx::query("SELECT id, username, hashed_password, role FROM users WHERE username = ?")
            .bind(username)
            .fetch_one(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(User {
            id: UserId(Uuid::parse_str(&row.get::<String, _>("id")).unwrap()),
            username: row.get::<String, _>("username"),
            hashed_password: row.get::<String, _>("hashed_password"),
            role: Role::Admin,
        })
    }

    async fn create_user(&self, user: &User) -> Result<(), DomainError> {
        sqlx::query("INSERT INTO users (id, username, hashed_password, role) VALUES (?, ?, ?, ?)")
            .bind(user.id.0.to_string())
            .bind(&user.username)
            .bind(&user.hashed_password)
            .bind("admin")
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;
        Ok(())
    }

    async fn update_user(&self, user: &User) -> Result<(), DomainError> {
        sqlx::query("UPDATE users SET username = ?, hashed_password = ? WHERE id = ?")
            .bind(&user.username)
            .bind(&user.hashed_password)
            .bind(user.id.0.to_string())
            .execute(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;
        Ok(())
    }

    async fn list_users(&self) -> Result<Vec<User>, DomainError> {
        let rows = sqlx::query("SELECT id, username, hashed_password, role FROM users")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| DomainError::Database(e.to_string()))?;

        Ok(rows.into_iter().map(|r| User {
            id: UserId(Uuid::parse_str(&r.get::<String, _>("id")).unwrap()),
            username: r.get::<String, _>("username"),
            hashed_password: r.get::<String, _>("hashed_password"),
            role: Role::Admin,
        }).collect())
    }
}
