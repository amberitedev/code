use std::sync::Arc;

use serde_json::Value;
use sqlx::QueryBuilder;
use uuid::Uuid;

use crate::application::{
    access_service,
    social_lookup_service::now,
    social_models::{
        ActivityLogEntry, ActivityLogQuery, ActivityLogResponse, SocialError,
    },
    state::AppState,
};

pub async fn record(
    state: &Arc<AppState>,
    actor_user_id: &str,
    action: &str,
    instance_id: Option<&str>,
    target_user_id: Option<&str>,
    metadata: Option<Value>,
) -> Result<(), SocialError> {
    let metadata_json = metadata
        .map(|value| serde_json::to_string(&value))
        .transpose()
        .map_err(|error| SocialError::Invalid(error.to_string()))?;
    sqlx::query("INSERT INTO activity_log (id, actor_user_id, action, instance_id, target_user_id, metadata_json, created_at) VALUES (?, ?, ?, ?, ?, ?, ?)")
		.bind(Uuid::new_v4().to_string())
		.bind(actor_user_id)
		.bind(action)
		.bind(instance_id)
		.bind(target_user_id)
		.bind(metadata_json)
		.bind(now())
		.execute(&state.pool)
		.await?;
    Ok(())
}

pub async fn list(
    state: &Arc<AppState>,
    query: ActivityLogQuery,
) -> Result<ActivityLogResponse, SocialError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let mut builder = QueryBuilder::new(
        "SELECT id, actor_user_id, action, instance_id, target_user_id, metadata_json, created_at FROM activity_log",
    );
    let mut has_where = false;

    macro_rules! push_filter {
        ($condition:literal, $value:expr) => {{
            if !has_where {
                builder.push(" WHERE ");
                has_where = true;
            } else {
                builder.push(" AND ");
            }
            builder.push($condition).push_bind($value);
        }};
    }

    if let Some(value) = query.instance_id {
        push_filter!("instance_id = ", value);
    }
    if let Some(value) = query.actor_user_id {
        push_filter!("actor_user_id = ", value);
    }
    if let Some(value) = query.target_user_id {
        push_filter!("target_user_id = ", value);
    }
    if let Some(value) = query.action {
        push_filter!("action = ", value);
    }
    if let Some(value) = query.min_datetime {
        push_filter!("created_at >= ", value);
    }
    if let Some(value) = query.max_datetime {
        push_filter!("created_at <= ", value);
    }
    if let Some(value) = query.cursor {
        push_filter!("created_at < ", value);
    }
    let _ = has_where;

    builder.push(" ORDER BY created_at DESC LIMIT ");
    builder.push_bind(limit + 1);

    let mut rows: Vec<ActivityLogEntry> =
        builder.build_query_as().fetch_all(&state.pool).await?;
    let next_cursor = if rows.len() as i64 > limit {
        rows.pop().map(|row| row.created_at)
    } else {
        None
    };

    Ok(ActivityLogResponse {
        entries: rows,
        next_cursor,
    })
}

pub async fn list_for_viewer(
    state: &Arc<AppState>,
    actor_user_id: &str,
    query: ActivityLogQuery,
) -> Result<ActivityLogResponse, SocialError> {
    let mut response = list(state, query).await?;
    let mut entries = Vec::with_capacity(response.entries.len());
    let can_view_core_events =
        access_service::require_core_member(state, actor_user_id)
            .await
            .is_ok();

    for entry in response.entries {
        if let Some(instance_id) = entry.instance_id.as_deref() {
            if access_service::require_instance_permission(
                state,
                actor_user_id,
                instance_id,
                "server:view",
            )
            .await
            .is_ok()
            {
                entries.push(entry);
            }
        } else if can_view_core_events {
            entries.push(entry);
        }
    }

    response.entries = entries;
    Ok(response)
}
