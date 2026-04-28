use async_trait::async_trait;
use sqlx::SqlitePool;

use crate::{
    domain::modpack::ModpackManifest,
    ports::{instance_store::StoreError, modpack_store::ModpackStore},
};

pub struct ModpackRepo {
    pool: SqlitePool,
}

impl ModpackRepo {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[derive(sqlx::FromRow)]
struct ManifestRow {
    id: String,
    instance_id: String,
    pack_name: String,
    pack_version: String,
    game_version: String,
    loader: String,
    loader_version: Option<String>,
    modrinth_project_id: Option<String>,
    modrinth_version_id: Option<String>,
    installed_at: String,
}

impl From<ManifestRow> for ModpackManifest {
    fn from(r: ManifestRow) -> Self {
        ModpackManifest {
            id: r.id,
            instance_id: r.instance_id,
            pack_name: r.pack_name,
            pack_version: r.pack_version,
            game_version: r.game_version,
            loader: r.loader,
            loader_version: r.loader_version,
            modrinth_project_id: r.modrinth_project_id,
            modrinth_version_id: r.modrinth_version_id,
            installed_at: r.installed_at,
        }
    }
}

#[async_trait]
impl ModpackStore for ModpackRepo {
    async fn save(&self, m: &ModpackManifest) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT OR REPLACE INTO modpack_manifests (id,instance_id,pack_name,pack_version,game_version,loader,loader_version,modrinth_project_id,modrinth_version_id,installed_at) VALUES (?,?,?,?,?,?,?,?,?,?)"
        )
        .bind(&m.id)
        .bind(&m.instance_id)
        .bind(&m.pack_name)
        .bind(&m.pack_version)
        .bind(&m.game_version)
        .bind(&m.loader)
        .bind(&m.loader_version)
        .bind(&m.modrinth_project_id)
        .bind(&m.modrinth_version_id)
        .bind(&m.installed_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn get_for_instance(&self, instance_id: &str) -> Result<Option<ModpackManifest>, StoreError> {
        let row = sqlx::query_as::<_, ManifestRow>(
            "SELECT * FROM modpack_manifests WHERE instance_id = ?"
        )
        .bind(instance_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Into::into))
    }

    async fn delete_for_instance(&self, instance_id: &str) -> Result<(), StoreError> {
        sqlx::query("DELETE FROM modpack_manifests WHERE instance_id = ?")
            .bind(instance_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
