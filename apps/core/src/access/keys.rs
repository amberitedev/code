//! The backend's public ticket keys ([`sharing_storage::keys`]), kept in Core's database.

use sharing_storage::keys::{Persist, PublicKeys};
use sqlx::SqlitePool;

pub type Keys = sharing_storage::keys::Keys<Db>;

/// The `core` table's `ticket_keys` and `ticket_keys_at`.
pub struct Db(SqlitePool);

impl Persist for Db {
    async fn save(
        &self,
        keys: &PublicKeys,
        fetched_at: i64,
    ) -> Result<(), String> {
        sqlx::query(
            "UPDATE core SET ticket_keys = ?, ticket_keys_at = ? WHERE id = 1",
        )
        .bind(serde_json::to_string(keys).expect("keys serialize"))
        .bind(fetched_at)
        .execute(&self.0)
        .await
        .map(drop)
        .map_err(|error| error.to_string())
    }
}

/// The keys stored in the database.
pub async fn load(
    db: SqlitePool,
    http: reqwest::Client,
    backend_url: Option<&str>,
) -> sqlx::Result<Keys> {
    let (stored, fetched_at): (Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT ticket_keys, ticket_keys_at FROM core WHERE id = 1",
    )
    .fetch_optional(&db)
    .await?
    .unwrap_or_default();
    let keys: PublicKeys = stored
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default();
    Ok(Keys::new(
        Db(db),
        &keys,
        fetched_at.unwrap_or(0),
        http,
        backend_url,
    ))
}

#[cfg(test)]
mod tests {
    use sharing_storage::keys::test_support::Signer;

    use super::*;
    use crate::db;

    #[tokio::test]
    async fn stored_keys_survive_a_restart() {
        let db = db::memory().await;
        db::core_id(&db).await.unwrap();
        let http = crate::network::http().build().unwrap();
        let signer = Signer::new("one");
        let keys = load(db.clone(), http.clone(), None).await.unwrap();
        keys.set(&signer.public_keys()).await;
        let again = load(db, http, None).await.unwrap();
        let ticket = signer.sign(&serde_json::json!({}));
        assert!(again.verify(&ticket).await.is_ok());
        let unknown = Signer::new("two").sign(&serde_json::json!({}));
        assert!(again.verify(&unknown).await.is_err());
    }
}
