//! The activity log: who did what, newest first. Written in the same transaction as the change it
//! records wherever there is one.

use sqlx::{SqliteConnection, SqlitePool};

use super::{Caller, now};
use crate::api::{
    access::User,
    activity::{Action, ActivityEntry, ActivityPage, ActivityQuery},
};

/// Record `action` by `user` (`None` for the local CLI) on `server` (`None` for the Core).
pub async fn record(
    conn: &mut SqliteConnection,
    user: Option<&str>,
    server: Option<&str>,
    action: &Action,
) -> sqlx::Result<()> {
    let mut detail = serde_json::to_value(action).expect("actions serialize");
    let kind = detail
        .as_object_mut()
        .and_then(|fields| fields.remove("type"))
        .and_then(|kind| kind.as_str().map(str::to_owned))
        .expect("actions are tagged");
    sqlx::query(
        "INSERT INTO activity (at, user_id, server_id, action, detail) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(now())
    .bind(user)
    .bind(server)
    .bind(kind)
    .bind(detail.to_string())
    .execute(conn)
    .await?;
    Ok(())
}

/// Record what `caller` did, outside any other write. A failure is logged, never shown: the
/// action already happened.
pub async fn log(
    db: &SqlitePool,
    caller: &Caller,
    server: Option<&str>,
    action: Action,
) {
    let recorded = async {
        let mut conn = db.acquire().await?;
        record(&mut conn, caller.user_id(), server, &action).await
    };
    if let Err(error) = recorded.await {
        tracing::warn!(%error, "could not record activity");
    }
}

/// A page of the log, newest first.
pub async fn page(
    db: &SqlitePool,
    query: &ActivityQuery,
) -> sqlx::Result<ActivityPage> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let rows: Vec<(i64, i64, Option<String>, Option<String>, Option<String>, Option<String>, String, String)> =
        sqlx::query_as(
            "SELECT a.id, a.at, a.user_id, u.name, u.avatar_url, a.server_id, a.action, a.detail \
             FROM activity a LEFT JOIN users u ON u.id = a.user_id \
             WHERE (?1 IS NULL OR a.server_id = ?1) AND (?2 IS NULL OR a.id < ?2) \
             ORDER BY a.id DESC LIMIT ?3",
        )
        .bind(query.server_id.map(|id| id.to_string()))
        .bind(query.before)
        .bind(i64::from(limit) + 1)
        .fetch_all(db)
        .await?;
    // The cursor is the last row read, so rows left out below never hide older ones.
    let next_before = (rows.len() > limit as usize)
        .then(|| rows.get(limit as usize - 1).map(|row| row.0))
        .flatten();
    let entries: Vec<ActivityEntry> = rows
        .into_iter()
        .take(limit as usize)
        .filter_map(
            |(id, at, user_id, name, avatar_url, server_id, kind, detail)| {
                let mut fields: serde_json::Map<String, serde_json::Value> =
                    serde_json::from_str(&detail).unwrap_or_default();
                fields.insert("type".into(), kind.into());
                Some(ActivityEntry {
                    id,
                    at,
                    user: user_id.map(|id| User {
                        name: name.unwrap_or_else(|| id.clone()),
                        id,
                        avatar_url,
                    }),
                    server_id: server_id.and_then(|id| id.parse().ok()),
                    // An action a newer Core wrote and this one does not know is left out.
                    action: serde_json::from_value(fields.into()).ok()?,
                })
            },
        )
        .collect();
    Ok(ActivityPage {
        next_before,
        entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{api::access::Role, db};

    #[tokio::test]
    async fn a_page_of_unknown_actions_still_leads_to_older_entries() {
        let db = db::memory().await;
        let mut conn = db.acquire().await.unwrap();
        record(&mut conn, None, None, &Action::EulaAccepted)
            .await
            .unwrap();
        drop(conn);
        // Written by a newer Core.
        sqlx::query(
            "INSERT INTO activity (at, action) VALUES (1, 'from_the_future'), (2, 'from_the_future')",
        )
        .execute(&db)
        .await
        .unwrap();
        let query = |before| ActivityQuery {
            server_id: None,
            before,
            limit: Some(2),
        };
        let first = page(&db, &query(None)).await.unwrap();
        assert!(first.entries.is_empty());
        let rest = page(&db, &query(first.next_before)).await.unwrap();
        assert!(matches!(
            rest.entries[..],
            [ActivityEntry {
                action: Action::EulaAccepted,
                ..
            }]
        ));
    }

    #[tokio::test]
    async fn entries_page_newest_first_with_their_details() {
        let db = db::memory().await;
        let server = uuid::Uuid::new_v4();
        let mut conn = db.acquire().await.unwrap();
        for user in ["a", "b", "c"] {
            record(
                &mut conn,
                None,
                Some(&server.to_string()),
                &Action::Invited {
                    user_id: user.into(),
                    role: Role::Editor,
                },
            )
            .await
            .unwrap();
        }
        record(&mut conn, Some("owner"), None, &Action::EulaAccepted)
            .await
            .unwrap();
        drop(conn);

        let first = page(
            &db,
            &ActivityQuery {
                server_id: Some(server),
                before: None,
                limit: Some(2),
            },
        )
        .await
        .unwrap();
        assert_eq!(first.entries.len(), 2);
        assert!(matches!(
            &first.entries[0].action,
            Action::Invited { user_id, role: Role::Editor } if user_id == "c"
        ));
        let rest = page(
            &db,
            &ActivityQuery {
                server_id: Some(server),
                before: first.next_before,
                limit: Some(2),
            },
        )
        .await
        .unwrap();
        assert_eq!(rest.entries.len(), 1);
        assert_eq!(rest.next_before, None);

        let all = page(
            &db,
            &ActivityQuery {
                server_id: None,
                before: None,
                limit: None,
            },
        )
        .await
        .unwrap();
        assert_eq!(all.entries.len(), 4);
        assert_eq!(all.entries[0].user.as_ref().unwrap().id, "owner");
    }
}
