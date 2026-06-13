use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;

use crate::{
    domain::instance::{
        InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus,
        MemorySettings, ModLoader,
    },
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
    jvm_args: Option<String>,
    server_args: Option<String>,
    install_status: String,
    status: String,
    data_dir: String,
    installation_id: Option<String>,
    total_uptime_seconds: i64,
    created_at: String,
    updated_at: String,
}

impl TryFrom<InstanceRow> for InstanceRecord {
    type Error = StoreError;
    fn try_from(r: InstanceRow) -> Result<Self, Self::Error> {
        Ok(InstanceRecord {
            id: r
                .id
                .parse::<uuid::Uuid>()
                .map(InstanceId)
                .map_err(|e| StoreError::Parse(e.to_string()))?,
            name: r.name,
            game_version: r.game_version,
            loader: r.loader.parse::<ModLoader>().map_err(StoreError::Parse)?,
            loader_version: r.loader_version,
            port: r.port as u16,
            memory: MemorySettings {
                min_mb: r.memory_min as u32,
                max_mb: r.memory_max as u32,
            },
            java_version: r.java_version,
            jvm_args: r.jvm_args,
            server_args: r.server_args,
            install_status: r
                .install_status
                .parse::<InstanceInstallStatus>()
                .map_err(StoreError::Parse)?,
            status: r
                .status
                .parse::<InstanceStatus>()
                .map_err(StoreError::Parse)?,
            data_dir: r.data_dir,
            installation_id: r.installation_id,
            total_uptime_seconds: r.total_uptime_seconds as u64,
            created_at: parse_timestamp(&r.created_at)?,
            updated_at: parse_timestamp(&r.updated_at)?,
        })
    }
}

/// BEH-09: Parse a timestamp that may be RFC 3339 ("2024-01-01T12:00:00Z") or SQLite's
/// CURRENT_TIMESTAMP format ("2024-01-01 12:00:00"). Rows written by `InstanceRepo::create`
/// use RFC 3339 explicitly; older rows inserted directly by SQL may use the SQLite format.
pub(crate) fn parse_timestamp(s: &str) -> Result<DateTime<Utc>, StoreError> {
    if let Ok(dt) = s.parse::<DateTime<Utc>>() {
        return Ok(dt);
    }
    chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%d %H:%M:%S")
        .map(|ndt| ndt.and_utc())
        .map_err(|e| StoreError::Parse(format!("invalid timestamp {s:?}: {e}")))
}

#[async_trait]
impl InstanceStore for InstanceRepo {
    async fn create(&self, r: &InstanceRecord) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO instances (id,name,game_version,loader,loader_version,port,memory_min,memory_max,java_version,jvm_args,server_args,install_status,status,data_dir,installation_id,total_uptime_seconds,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)"
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
        .bind(&r.jvm_args)
        .bind(&r.server_args)
        .bind(r.install_status.to_string())
        .bind(r.status.to_string())
        .bind(&r.data_dir)
        .bind(&r.installation_id)
        .bind(r.total_uptime_seconds as i64)
        .bind(r.created_at.to_rfc3339())
        .bind(r.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(&self, id: &InstanceId) -> Result<InstanceRecord, StoreError> {
        let row = sqlx::query_as::<_, InstanceRow>(
            "SELECT * FROM instances WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?
        .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        row.try_into()
    }

    async fn list(&self) -> Result<Vec<InstanceRecord>, StoreError> {
        let rows = sqlx::query_as::<_, InstanceRow>(
            "SELECT * FROM instances ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn update_status(
        &self,
        id: &InstanceId,
        status: InstanceStatus,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET status = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status.to_string())
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_port(
        &self,
        id: &InstanceId,
        port: u16,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET port = ?, updated_at = ? WHERE id = ?",
        )
        .bind(port as i64)
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_name(
        &self,
        id: &InstanceId,
        name: &str,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET name = ?, updated_at = ? WHERE id = ?",
        )
        .bind(name)
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_java_version(
        &self,
        id: &InstanceId,
        java_version: Option<i64>,
    ) -> Result<(), StoreError> {
        let result =
            sqlx::query("UPDATE instances SET java_version = ?, updated_at = ? WHERE id = ?")
                .bind(java_version)
                .bind(Utc::now().to_rfc3339())
                .bind(id.to_string())
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_memory(
        &self,
        id: &InstanceId,
        min_mb: u32,
        max_mb: u32,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET memory_min = ?, memory_max = ?, updated_at = ? WHERE id = ?",
        )
        .bind(min_mb as i64)
        .bind(max_mb as i64)
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_startup(
        &self,
        id: &InstanceId,
        jvm_args: Option<&str>,
        server_args: Option<&str>,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET jvm_args = ?, server_args = ?, updated_at = ? WHERE id = ?",
        )
        .bind(jvm_args)
        .bind(server_args)
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_version(
        &self,
        id: &InstanceId,
        game_version: &str,
        loader: &ModLoader,
        loader_version: Option<&str>,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET game_version = ?, loader = ?, loader_version = ?, updated_at = ? WHERE id = ?",
        )
        .bind(game_version)
        .bind(loader.to_string())
        .bind(loader_version)
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_install_status(
        &self,
        id: &InstanceId,
        install_status: InstanceInstallStatus,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET install_status = ?, updated_at = ? WHERE id = ?",
        )
        .bind(install_status.to_string())
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn update_installation_id(
        &self,
        id: &InstanceId,
        installation_id: Option<&str>,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET installation_id = ?, updated_at = ? WHERE id = ?",
        )
        .bind(installation_id)
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn delete(&self, id: &InstanceId) -> Result<(), StoreError> {
        let result = sqlx::query("DELETE FROM instances WHERE id = ?")
            .bind(id.to_string())
            .execute(&self.pool)
            .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn list_by_status(
        &self,
        status: InstanceStatus,
    ) -> Result<Vec<InstanceRecord>, StoreError> {
        let rows = sqlx::query_as::<_, InstanceRow>(
            "SELECT * FROM instances WHERE status = ?",
        )
        .bind(status.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn list_by_installation(
        &self,
        installation_id: &str,
    ) -> Result<Vec<InstanceRecord>, StoreError> {
        let rows = sqlx::query_as::<_, InstanceRow>(
            "SELECT * FROM instances WHERE installation_id = ?",
        )
        .bind(installation_id)
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(|r| r.try_into()).collect()
    }

    async fn reset_transient_statuses(&self) -> Result<u64, StoreError> {
        let result = sqlx::query(
            "UPDATE instances SET status = 'offline' WHERE status IN ('starting', 'stopping')",
        )
        .execute(&self.pool)
        .await?;
        Ok(result.rows_affected())
    }

    async fn add_uptime(
        &self,
        id: &InstanceId,
        seconds: u64,
    ) -> Result<(), StoreError> {
        let secs = seconds as i64;
        let id_str = id.to_string();
        sqlx::query(
            "UPDATE instances SET total_uptime_seconds = total_uptime_seconds + ?1 WHERE id = ?2",
        )
        .bind(secs)
        .bind(id_str)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
