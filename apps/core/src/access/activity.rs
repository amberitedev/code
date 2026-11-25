//! The activity log: who did what. Written in the same transaction as the change it records
//! wherever there is one.

use contracts::{
    ApiError, ErrorCode,
    core::{
        Action, ActionEntry, ActionLogFilter, ActionLogResponse, ActionUser,
        ActivityQuery, SortOrder, UserResp,
    },
};
use sqlx::{QueryBuilder, Sqlite, SqliteConnection, SqlitePool};

use super::{Caller, at, now};

/// Record `action` by `user` (`None` for the local CLI) on `server` (`None` for the Core).
pub async fn record(
    conn: &mut SqliteConnection,
    user: Option<&str>,
    server: Option<&str>,
    action: &Action,
) -> sqlx::Result<()> {
    let mut fields = serde_json::to_value(action).expect("actions serialize");
    let fields = fields.as_object_mut().expect("actions are tagged");
    let name = fields
        .remove("action")
        .and_then(|name| name.as_str().map(str::to_owned))
        .expect("actions are tagged");
    let metadata = fields.remove("metadata").unwrap_or_default();
    sqlx::query(
        "INSERT INTO activity (at, user_id, server_id, action, detail) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(now())
    .bind(user)
    .bind(server)
    .bind(name)
    .bind(metadata.to_string())
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

type Row = (
    i64,
    Option<String>,
    Option<String>,
    Option<String>,
    Option<String>,
    String,
    String,
);

/// The page of the log `query` asks for, filtered here rather than by the caller.
pub async fn page(
    db: &SqlitePool,
    query: &ActivityQuery,
) -> Result<ActionLogResponse, ApiError> {
    let filter: ActionLogFilter = match &query.filter {
        Some(json) => serde_json::from_str(json).map_err(|error| {
            ApiError::new(
                ErrorCode::InvalidInput,
                format!("The filter is not an action log filter: {error}"),
            )
        })?,
        None => ActionLogFilter::default(),
    };
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let offset = query.offset.unwrap_or(0);
    let mut sql: QueryBuilder<Sqlite> = QueryBuilder::new(
        "SELECT a.at, a.user_id, u.name, u.avatar_url, a.server_id, a.action, a.detail \
         FROM activity a LEFT JOIN users u ON u.id = a.user_id WHERE 1",
    );
    if let Some(server) = query.server_id {
        sql.push(" AND a.server_id = ")
            .push_bind(server.to_string());
    }
    if let Some(min) = query.min_datetime {
        sql.push(" AND a.at >= ").push_bind(min.timestamp());
    }
    if let Some(max) = query.max_datetime {
        sql.push(" AND a.at <= ").push_bind(max.timestamp());
    }
    for (column, values) in
        [("a.user_id", &filter.users), ("a.action", &filter.actions)]
    {
        let Some(values) = values else { continue };
        sql.push(format!(" AND {column} IN ("));
        // NULL matches nothing, and keeps an empty list valid SQL.
        let mut list = sql.separated(", ");
        list.push("NULL");
        for value in values {
            list.push_bind(value);
        }
        sql.push(")");
    }
    sql.push(match query.order {
        Some(SortOrder::Asc) => " ORDER BY a.id ASC",
        Some(SortOrder::Desc) | None => " ORDER BY a.id DESC",
    });
    sql.push(" LIMIT ")
        .push_bind(i64::from(limit) + 1)
        .push(" OFFSET ")
        .push_bind(i64::from(offset));
    let rows: Vec<Row> = sql
        .build_query_as()
        .fetch_all(db)
        .await
        .map_err(crate::api::internal)?;
    let next_offset = (rows.len() > limit as usize).then_some(offset + limit);
    let mut users = std::collections::BTreeMap::new();
    let data = rows
        .into_iter()
        .take(limit as usize)
        .filter_map(
            |(
                seconds,
                user_id,
                name,
                avatar_url,
                server_id,
                name_of,
                detail,
            )| {
                let metadata: serde_json::Value =
                    serde_json::from_str(&detail).unwrap_or_default();
                let mut action = serde_json::Map::new();
                action.insert("action".into(), name_of.into());
                if !metadata.is_null() {
                    action.insert("metadata".into(), metadata);
                }
                let actor = match user_id {
                    Some(user_id) => {
                        users.entry(user_id.clone()).or_insert_with(|| {
                            UserResp {
                                username: name
                                    .unwrap_or_else(|| user_id.clone()),
                                avatar_url,
                            }
                        });
                        ActionUser::User { user_id }
                    }
                    None => ActionUser::Cli,
                };
                Some(ActionEntry {
                    actor,
                    // An action a newer Core wrote and this one does not know is left out.
                    action: serde_json::from_value(action.into()).ok()?,
                    server_id,
                    timestamp: at(seconds),
                })
            },
        )
        .collect();
    Ok(ActionLogResponse {
        data,
        next_offset,
        users,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::db;

    async fn console_commands(
        db: &SqlitePool,
        server: &str,
        user: Option<&str>,
    ) {
        let mut conn = db.acquire().await.unwrap();
        for command in ["a", "b", "c"] {
            record(
                &mut conn,
                user,
                Some(server),
                &Action::ConsoleCommandExecuted {
                    command: command.into(),
                },
            )
            .await
            .unwrap();
        }
    }

    #[tokio::test]
    async fn entries_page_newest_first_with_their_details() {
        let db = db::memory().await;
        let server = uuid::Uuid::new_v4();
        console_commands(&db, &server.to_string(), None).await;
        let mut conn = db.acquire().await.unwrap();
        record(&mut conn, Some("owner"), None, &Action::EulaAccepted)
            .await
            .unwrap();
        drop(conn);

        let query = |offset| ActivityQuery {
            server_id: Some(server),
            limit: Some(2),
            offset,
            ..Default::default()
        };
        let first = page(&db, &query(None)).await.unwrap();
        assert_eq!(first.data.len(), 2);
        assert!(matches!(
            &first.data[0],
            ActionEntry {
                actor: ActionUser::Cli,
                action: Action::ConsoleCommandExecuted { command },
                ..
            } if command == "c"
        ));
        let rest = page(&db, &query(first.next_offset)).await.unwrap();
        assert_eq!(rest.data.len(), 1);
        assert_eq!(rest.next_offset, None);

        let all = page(&db, &ActivityQuery::default()).await.unwrap();
        assert_eq!(all.data.len(), 4);
        assert_eq!(
            all.data[0].actor,
            ActionUser::User {
                user_id: "owner".into()
            }
        );
        assert_eq!(all.users["owner"].username, "owner");
    }

    #[tokio::test]
    async fn the_core_filters_by_action_user_time_and_order() {
        let db = db::memory().await;
        let server = uuid::Uuid::new_v4().to_string();
        console_commands(&db, &server, Some("owner")).await;
        let mut conn = db.acquire().await.unwrap();
        record(&mut conn, None, Some(&server), &Action::ServerStarted)
            .await
            .unwrap();
        // Written by a newer Core.
        sqlx::query("INSERT INTO activity (at, action, detail) VALUES (1, 'from_the_future', 'null')")
            .execute(&mut *conn)
            .await
            .unwrap();
        drop(conn);

        let listed = |filter: ActionLogFilter, order| {
            let db = db.clone();
            async move {
                page(
                    &db,
                    &ActivityQuery {
                        filter: Some(serde_json::to_string(&filter).unwrap()),
                        order,
                        ..Default::default()
                    },
                )
                .await
                .unwrap()
                .data
            }
        };
        let started = listed(
            ActionLogFilter {
                actions: Some(vec!["server_started".into()]),
                ..Default::default()
            },
            None,
        )
        .await;
        assert!(matches!(
            started[..],
            [ActionEntry {
                action: Action::ServerStarted,
                ..
            }]
        ));

        let by_owner = listed(
            ActionLogFilter {
                users: Some(vec!["owner".into()]),
                ..Default::default()
            },
            Some(SortOrder::Asc),
        )
        .await;
        assert_eq!(by_owner.len(), 3);
        assert!(matches!(
            &by_owner[0].action,
            Action::ConsoleCommandExecuted { command } if command == "a"
        ));

        let since = page(
            &db,
            &ActivityQuery {
                min_datetime: Some(at(2)),
                ..Default::default()
            },
        )
        .await
        .unwrap();
        assert_eq!(since.data.len(), 4, "everything but the entry at 1");

        let bad = page(
            &db,
            &ActivityQuery {
                filter: Some("not json".into()),
                ..Default::default()
            },
        )
        .await;
        assert_eq!(bad.unwrap_err().error, ErrorCode::InvalidInput);
    }
}
