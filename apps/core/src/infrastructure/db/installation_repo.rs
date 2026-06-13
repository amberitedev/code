use async_trait::async_trait;
use chrono::Utc;
use sqlx::SqlitePool;

use crate::{
    domain::instance::ModLoader,
    domain::server_installation::{
        InstallationId, InstallationStatus, ServerInstallationRecord,
    },
    infrastructure::db::instance_repo::parse_timestamp,
    ports::{
        installation_store::InstallationStore, instance_store::StoreError,
    },
};

pub struct InstallationRepo {
    pool: SqlitePool,
}

impl InstallationRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct InstallationRow {
    id: String,
    game_version: String,
    loader: String,
    loader_version: Option<String>,
    status: String,
    error: Option<String>,
    created_at: String,
    updated_at: String,
}

impl TryFrom<InstallationRow> for ServerInstallationRecord {
    type Error = StoreError;
    fn try_from(r: InstallationRow) -> Result<Self, Self::Error> {
        Ok(ServerInstallationRecord {
            id: InstallationId(r.id),
            game_version: r.game_version,
            loader: r.loader.parse::<ModLoader>().map_err(StoreError::Parse)?,
            loader_version: r.loader_version,
            status: r
                .status
                .parse::<InstallationStatus>()
                .map_err(StoreError::Parse)?,
            error: r.error,
            created_at: parse_timestamp(&r.created_at)?,
            updated_at: parse_timestamp(&r.updated_at)?,
        })
    }
}

#[async_trait]
impl InstallationStore for InstallationRepo {
    async fn create(
        &self,
        r: &ServerInstallationRecord,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO server_installations (id,game_version,loader,loader_version,status,error,created_at,updated_at) VALUES (?,?,?,?,?,?,?,?)"
        )
        .bind(r.id.to_string())
        .bind(&r.game_version)
        .bind(r.loader.to_string())
        .bind(&r.loader_version)
        .bind(r.status.to_string())
        .bind(&r.error)
        .bind(r.created_at.to_rfc3339())
        .bind(r.updated_at.to_rfc3339())
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get(
        &self,
        id: &InstallationId,
    ) -> Result<Option<ServerInstallationRecord>, StoreError> {
        let row = sqlx::query_as::<_, InstallationRow>(
            "SELECT * FROM server_installations WHERE id = ?",
        )
        .bind(id.to_string())
        .fetch_optional(&self.pool)
        .await?;
        row.map(TryInto::try_into).transpose()
    }

    async fn list(&self) -> Result<Vec<ServerInstallationRecord>, StoreError> {
        let rows = sqlx::query_as::<_, InstallationRow>(
            "SELECT * FROM server_installations ORDER BY created_at",
        )
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn list_by_status(
        &self,
        status: InstallationStatus,
    ) -> Result<Vec<ServerInstallationRecord>, StoreError> {
        let rows = sqlx::query_as::<_, InstallationRow>(
            "SELECT * FROM server_installations WHERE status = ?",
        )
        .bind(status.to_string())
        .fetch_all(&self.pool)
        .await?;
        rows.into_iter().map(TryInto::try_into).collect()
    }

    async fn update_status(
        &self,
        id: &InstallationId,
        status: InstallationStatus,
        error: Option<&str>,
    ) -> Result<(), StoreError> {
        let result = sqlx::query(
            "UPDATE server_installations SET status = ?, error = ?, updated_at = ? WHERE id = ?",
        )
        .bind(status.to_string())
        .bind(error)
        .bind(Utc::now().to_rfc3339())
        .bind(id.to_string())
        .execute(&self.pool)
        .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }

    async fn delete(&self, id: &InstallationId) -> Result<(), StoreError> {
        let result =
            sqlx::query("DELETE FROM server_installations WHERE id = ?")
                .bind(id.to_string())
                .execute(&self.pool)
                .await?;
        if result.rows_affected() == 0 {
            return Err(StoreError::NotFound(id.to_string()));
        }
        Ok(())
    }
}
