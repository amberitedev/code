//! SQLite-backed [`RelayStore`] over the `core_relay_messages` table, plus the
//! [`StoredMessage`] row it reads and writes. This is the only place that touches
//! relay SQL.

use async_trait::async_trait;
use serde::Serialize;
use serde_json::Value;
use sqlx::SqlitePool;

use super::audience::Endpoint;
use super::envelope::Envelope;
use super::error::ApiCommError;
use super::ids::MessageId;
use super::status::DeliveryStatus;
use super::store::RelayStore;

/// One row of `core_relay_messages`, in the wire shape the app already expects.
/// `result`/`error` are only populated by [`SqliteRelayStore::get`] (the status
/// endpoint); they stay absent from the `pending` list shape.
#[derive(Debug, Clone, Serialize)]
pub struct StoredMessage {
    pub id: String,
    pub r#type: String,
    pub version: i64,
    pub sender_id: String,
    pub recipient_id: String,
    pub payload: Value,
    pub ack: String,
    pub status: String,
    pub created_at: String,
    pub expires_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

type Row = (
    String,
    String,
    i64,
    String,
    String,
    String,
    String,
    String,
    String,
    String,
);

fn row_to_message(row: Row) -> StoredMessage {
    StoredMessage {
        id: row.0,
        r#type: row.1,
        version: row.2,
        sender_id: row.3,
        recipient_id: row.4,
        payload: serde_json::from_str(&row.5).unwrap_or(Value::Null),
        ack: row.6,
        status: row.7,
        created_at: row.8,
        expires_at: row.9,
        result: None,
        error: None,
    }
}

/// SQLite implementation of [`RelayStore`].
pub struct SqliteRelayStore {
    pool: SqlitePool,
}

impl SqliteRelayStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl RelayStore for SqliteRelayStore {
    async fn enqueue(
        &self,
        recipient: &Endpoint,
        envelope: &Envelope,
    ) -> Result<(), ApiCommError> {
        let expires_at = envelope
            .expires_at
            .unwrap_or(envelope.created_at)
            .to_rfc3339();
        let message = StoredMessage {
            id: envelope.id.to_string(),
            r#type: envelope.kind.wire().to_string(),
            version: envelope.version as i64,
            sender_id: envelope
                .origin
                .as_ref()
                .map(|o| o.to_string())
                .unwrap_or_default(),
            recipient_id: recipient.recipient_id().to_string(),
            payload: envelope.payload.clone(),
            ack: envelope.ack.wire().to_string(),
            status: DeliveryStatus::Pending.wire().to_string(),
            created_at: envelope.created_at.to_rfc3339(),
            expires_at,
            result: None,
            error: None,
        };
        self.insert(&message).await
    }

    async fn insert(
        &self,
        message: &StoredMessage,
    ) -> Result<(), ApiCommError> {
        sqlx::query(
            "INSERT INTO core_relay_messages \
             (id, type, version, sender_id, recipient_id, payload, ack, status, created_at, expires_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(&message.id)
        .bind(&message.r#type)
        .bind(message.version)
        .bind(&message.sender_id)
        .bind(&message.recipient_id)
        .bind(message.payload.to_string())
        .bind(&message.ack)
        .bind(&message.status)
        .bind(&message.created_at)
        .bind(&message.expires_at)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn pending(
        &self,
        recipient_id: &str,
    ) -> Result<Vec<StoredMessage>, ApiCommError> {
        let now = chrono::Utc::now().to_rfc3339();
        let rows: Vec<Row> = sqlx::query_as(
            "SELECT id, type, version, sender_id, recipient_id, payload, ack, status, created_at, expires_at \
             FROM core_relay_messages \
             WHERE recipient_id = ? AND status = 'pending' AND expires_at > ? \
             ORDER BY created_at ASC LIMIT 100",
        )
        .bind(recipient_id)
        .bind(now)
        .fetch_all(&self.pool)
        .await?;

        Ok(rows.into_iter().map(row_to_message).collect())
    }

    async fn get(
        &self,
        id: &MessageId,
    ) -> Result<Option<StoredMessage>, ApiCommError> {
        type StatusRow = (
            String,
            String,
            i64,
            String,
            String,
            String,
            String,
            String,
            String,
            String,
            Option<String>,
            Option<String>,
        );
        let row: Option<StatusRow> = sqlx::query_as(
            "SELECT id, type, version, sender_id, recipient_id, payload, ack, status, created_at, expires_at, result, error \
             FROM core_relay_messages WHERE id = ?",
        )
        .bind(id.as_str())
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| StoredMessage {
            id: row.0,
            r#type: row.1,
            version: row.2,
            sender_id: row.3,
            recipient_id: row.4,
            payload: serde_json::from_str(&row.5).unwrap_or(Value::Null),
            ack: row.6,
            status: row.7,
            created_at: row.8,
            expires_at: row.9,
            result: row.10.and_then(|v| serde_json::from_str::<Value>(&v).ok()),
            error: row.11,
        }))
    }

    async fn mark(
        &self,
        id: &MessageId,
        recipient_id: &str,
        status: DeliveryStatus,
        result: Option<Value>,
        error: Option<String>,
    ) -> Result<(), ApiCommError> {
        let now = chrono::Utc::now().to_rfc3339();
        let result = result.map(|v| v.to_string());

        match status {
            DeliveryStatus::Received => {
                sqlx::query(
                    "UPDATE core_relay_messages SET status = ?, received_at = ?, result = ?, error = ? \
                     WHERE id = ? AND recipient_id = ?",
                )
                .bind(status.wire())
                .bind(now)
                .bind(result)
                .bind(error)
                .bind(id.as_str())
                .bind(recipient_id)
                .execute(&self.pool)
                .await?;
            }
            DeliveryStatus::Processed => {
                sqlx::query(
                    "UPDATE core_relay_messages SET status = ?, processed_at = ?, result = ?, error = ? \
                     WHERE id = ? AND recipient_id = ?",
                )
                .bind(status.wire())
                .bind(now)
                .bind(result)
                .bind(error)
                .bind(id.as_str())
                .bind(recipient_id)
                .execute(&self.pool)
                .await?;
            }
            _ => {
                sqlx::query(
                    "UPDATE core_relay_messages SET status = ?, result = ?, error = ? \
                     WHERE id = ? AND recipient_id = ?",
                )
                .bind(status.wire())
                .bind(result)
                .bind(error)
                .bind(id.as_str())
                .bind(recipient_id)
                .execute(&self.pool)
                .await?;
            }
        }
        Ok(())
    }
}
