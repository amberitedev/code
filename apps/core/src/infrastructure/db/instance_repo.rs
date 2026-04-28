use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::{
    domain::instance::{InstanceId, InstanceRecord, InstanceStatus, MemorySettings, ModLoader},
    ports::instance_store::{InstanceStore, StoreError},
};

pub struct InstanceRepo {
    pool: SqlitePool,
}

impl InstanceRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

/// Flat row for SQLite deserialization.
#[derive(sqlx::FromRow)]
struct InstanceRow {
    id: String,
    name: String,
    game_version: String,
    loader: String,
    loader_version: Option<String>,
    port: i64,
    memory_min: i64,
    memory_max: i64,
    java_version: Option<i64>,
    status: String,
    data_dir: String,
    created_at: String,
    updated_at: String,
}

impl TryFrom<InstanceRow> for InstanceRecord {
    type Error = StoreError;
    fn try_from(r: InstanceRow) -> Result<Self, Self::Error> {
        Ok(InstanceRecord {
            id: r.id.parse::<uuid::Uuid>().map(InstanceId).map_err(|e| StoreError::Parse(e.to_string()))?,
            name: r.name,
            game_version: r.game_version,
            loader: r.loader.parse::<ModLoader>().map_err(StoreError::Parse)?,
            loader_version: r.loader_version,
            port: r.port as u16,
            memory: MemorySettings { min_mb: r.memory_min as u32, max_mb: r.memory_max as u32 },
            java_version: r.java_version,
            status: r.status.parse::<InstanceStatus>().map_err(StoreError::Parse)?,
            data_dir: r.data_dir,
            created_at: r.created_at.parse::<DateTime<Utc>>().map_err(|e| StoreError::Parse(e.to_string()))?,
            updated_at: r.updated_at.parse::<DateTime<Utc>>().map_err(|e| StoreError::Parse(e.to_string()))?,
        })
    }
}

#[async_trait]
impl InstanceStore for InstanceRepo {
    async fn create(&self, r: &InstanceRecord) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO instances (id,name,game_version,loader,loader_version,port,memory_min,memory_max,java_version,status,data_dir,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)"
        )
        .bind(r.id.to_string())
        .bind(&r.name)
        .bind(&r.game_version)
        .bind(r.loader.to_string())
        .bind(&r.loader_version)
        .bind(r.port as i64)
        .bind(r.memory.min_mb as i64)
        .bind(r.memory.max_mb as i64)
        .bind(r.java_version)
        .bind(r.status.to_string())
        .bind(&r.data_dir)
        .bind(r.created_at.to_rfc3339())
        .bind(r.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, id: &InstanceId) -> Result<InstanceRecord, StoreError> {
        let row = sqlx::query_as::<_, InstanceRow>(
            "SELECT * FROM instances WHERE id = ?"
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        row.try_into()
    }

    async fn list(&self) -> Result<Vec<InstanceRecord>, StoreError> {
        let rows = sqlx::query_as::<_, InstanceRow>("SELECT * FROM instances ORDER BY created_at")
            .fetch_all(&self.pool)
            .await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn update_status(&self, id: &InstanceId, status: InstanceStatus) -> Result<(), StoreError> {
        sqlx::query("UPDATE instances SET status = ?, updated_at = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(Utc::now().to_rfc3339())
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: &InstanceId) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM instances WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn list_by_status(&self, status: InstanceStatus) -> Result<Vec<InstanceRecord>, StoreError> {
        let rows = sqlx::query_as::<_, InstanceRow>(
            "SELECT * FROM instances WHERE status = ?"
        )
        .bind(status.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }
}
