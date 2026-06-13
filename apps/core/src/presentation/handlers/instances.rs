use std::sync::Arc;

use axum::{
    extract::{Path, State},
    http::StatusCode,
    Json,
};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{
    application::{
        access_service, activity_service,
        instance_service::{
            create_instance as svc_create_instance, CreateInstanceRequest,
        },
        instance_status_service::{default_launch_args, resolve_launch},
        state::AppState,
    },
    domain::{
        event::Event,
        instance::{InstanceId, InstanceRecord, MemorySettings, ModLoader},
    },
    presentation::{
        authz::{can_access_instance, require_instance_permission},
        error::ApiError,
        extractors::AuthUser,
    },
};

#[derive(Deserialize)]
pub struct CreateBody {
    pub name: String,
    pub game_version: String,
    pub loader: ModLoader,
    pub loader_version: Option<String>,
    pub port: u16,
    pub memory: Option<MemorySettings>,
}

pub(crate) fn record_list_item(r: &InstanceRecord) -> Value {
    json!({
        "id": r.id.to_string(), "name": r.name, "game_version": r.game_version,
        "loader": r.loader.to_string(), "loader_version": r.loader_version,
        "port": r.port, "memory": { "min_mb": r.memory.min_mb, "max_mb": r.memory.max_mb },
        "status": r.status.to_string(), "install_status": r.install_status.to_string(),
        "installation_id": r.installation_id,
        "created_at": r.created_at, "updated_at": r.updated_at,
    })
}

fn record_detail(r: &InstanceRecord) -> Value {
    json!({
        "id": r.id.to_string(), "name": r.name, "game_version": r.game_version,
        "loader": r.loader.to_string(), "loader_version": r.loader_version,
        "port": r.port, "memory": { "min_mb": r.memory.min_mb, "max_mb": r.memory.max_mb },
        "java_version": r.java_version, "jvm_args": r.jvm_args, "server_args": r.server_args,
        "install_status": r.install_status.to_string(),
        "status": r.status.to_string(),
        "installation_id": r.installation_id,
        "data_dir": r.data_dir, "created_at": r.created_at, "updated_at": r.updated_at,
    })
}

/// GET /instances — list all instances.
pub async fn list_instances(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let records = state.instance_store.list().await?;
    let mut instances = Vec::new();
    for record in records {
        let id = record.id.to_string();
        if can_access_instance(&state, &claims.sub, &id, "server:view").await {
            instances.push(record_list_item(&record));
        }
    }
    Ok(Json(json!({ "instances": instances })))
}

/// GET /instances/:id — get a single instance.
pub async fn get_instance(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = id.parse::<InstanceId>().map_err(|_| {
        ApiError::BadRequest("invalid instance id — must be a UUID".into())
    })?;
    require_instance_permission(&state, &claims.sub, &id, "server:view")
        .await?;
    let record = state.instance_store.get(&iid).await?;
    Ok(Json(record_detail(&record)))
}

/// POST /instances — create a new instance (JAR download is async).
pub async fn create_instance(
    AuthUser(claims): AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateBody>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    if body.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name cannot be empty".into()));
    }
    if body.port == 0 {
        return Err(ApiError::BadRequest("port cannot be 0".into()));
    }
    access_service::require_core_manager(&state, &claims.sub)
        .await
        .map_err(|error| ApiError::Forbidden(error.to_string()))?;
    let req = CreateInstanceRequest {
        name: body.name,
        game_version: body.game_version,
        loader: body.loader,
        loader_version: body.loader_version,
        port: body.port,
        memory: body.memory.unwrap_or_default(),
    };
    let id = svc_create_instance(&state, req).await?;
    let record = state.instance_store.get(&id).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "instance_created",
        Some(&id.to_string()),
        None,
        Some(json!({ "name": record.name })),
    )
    .await?;
    Ok((StatusCode::CREATED, Json(record_detail(&record))))
}

/// PATCH /instances/:id — update mutable instance fields.
///
/// All fields are optional. `java_version: null` clears the override; likewise
/// `jvm_args: null` / `server_args: null` clear those launch overrides.
#[derive(Deserialize)]
pub struct PatchBody {
    pub name: Option<String>,
    #[serde(default, deserialize_with = "deserialize_optional_option")]
    pub java_version: Option<Option<i64>>,
    pub memory: Option<MemorySettings>,
    #[serde(default, deserialize_with = "deserialize_optional_option")]
    pub jvm_args: Option<Option<String>>,
    #[serde(default, deserialize_with = "deserialize_optional_option")]
    pub server_args: Option<Option<String>>,
}

/// Deserializes a JSON field that distinguishes between absent (`None`) and
/// explicit `null` (`Some(None)`). Required because `#[serde(default)]` alone
/// maps both absent and null to `None`.
fn deserialize_optional_option<'de, D, T>(
    d: D,
) -> Result<Option<Option<T>>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: Deserialize<'de>,
{
    Ok(Some(Option::<T>::deserialize(d)?))
}

/// Trims a freshly-supplied override string, mapping blank input to `None`
/// (which clears the column).
fn normalize_override(value: Option<String>) -> Option<String> {
    value.and_then(|s| {
        let trimmed = s.trim().to_string();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed)
        }
    })
}

pub async fn patch_instance(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
    Json(body): Json<PatchBody>,
) -> Result<Json<Value>, ApiError> {
    let iid = id.parse::<InstanceId>().map_err(|_| {
        ApiError::BadRequest("invalid instance id — must be a UUID".into())
    })?;
    require_instance_permission(&state, &claims.sub, &id, "server:settings")
        .await?;

    if let Some(ref name) = body.name {
        if name.trim().is_empty() {
            return Err(ApiError::BadRequest("name cannot be empty".into()));
        }
        state.instance_store.update_name(&iid, name).await?;
    }

    if let Some(java_version) = body.java_version {
        state
            .instance_store
            .update_java_version(&iid, java_version)
            .await?;
    }

    if let Some(memory) = body.memory {
        if memory.min_mb == 0 || memory.max_mb == 0 {
            return Err(ApiError::BadRequest(
                "memory must be greater than 0".into(),
            ));
        }
        if memory.min_mb > memory.max_mb {
            return Err(ApiError::BadRequest(
                "memory min_mb cannot exceed max_mb".into(),
            ));
        }
        state
            .instance_store
            .update_memory(&iid, memory.min_mb, memory.max_mb)
            .await?;
    }

    if body.jvm_args.is_some() || body.server_args.is_some() {
        let current = state.instance_store.get(&iid).await?;
        let jvm = match body.jvm_args {
            Some(value) => normalize_override(value),
            None => current.jvm_args,
        };
        let server = match body.server_args {
            Some(value) => normalize_override(value),
            None => current.server_args,
        };
        state
            .instance_store
            .update_startup(&iid, jvm.as_deref(), server.as_deref())
            .await?;
    }

    let record = state.instance_store.get(&iid).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "instance_updated",
        Some(&iid.to_string()),
        None,
        Some(json!({ "name": record.name })),
    )
    .await?;
    state.broadcaster.send(Event::InstanceUpdated {
        instance: record.clone(),
    });
    Ok(Json(record_detail(&record)))
}

/// GET /instances/:id/startup — current launch tuning plus the rendered default
/// and effective commands, so the Advanced settings tab can show users exactly
/// what Core runs and offer a reset baseline.
pub async fn get_startup(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    let iid = id.parse::<InstanceId>().map_err(|_| {
        ApiError::BadRequest("invalid instance id — must be a UUID".into())
    })?;
    require_instance_permission(&state, &claims.sub, &id, "server:settings")
        .await?;
    let record = state.instance_store.get(&iid).await?;
    let (def_java, def_args) = default_launch_args(&state, &record).await;
    let (eff_java, eff_args) = resolve_launch(&state, &record).await;
    Ok(Json(json!({
        "java_version": record.java_version,
        "memory": { "min_mb": record.memory.min_mb, "max_mb": record.memory.max_mb },
        "jvm_args": record.jvm_args,
        "server_args": record.server_args,
        "default_command": render_command(&def_java, &def_args),
        "effective_command": render_command(&eff_java, &eff_args),
    })))
}

/// Joins a Java binary path and argument list into a single display string,
/// quoting any token that contains whitespace.
fn render_command(java: &std::path::Path, args: &[String]) -> String {
    let mut parts = vec![quote_if_needed(&java.display().to_string())];
    parts.extend(args.iter().map(|a| quote_if_needed(a)));
    parts.join(" ")
}

fn quote_if_needed(token: &str) -> String {
    if token.chars().any(char::is_whitespace) {
        format!("\"{token}\"")
    } else {
        token.to_string()
    }
}

/// DELETE /instances/:id — delete an instance (must be offline).
pub async fn delete_instance(
    AuthUser(claims): AuthUser,
    Path(id): Path<String>,
    State(state): State<Arc<AppState>>,
) -> Result<Json<Value>, ApiError> {
    // SEC-04: reject non-UUID paths before touching the database.
    let iid = id.parse::<InstanceId>().map_err(|_| {
        ApiError::BadRequest("invalid instance id — must be a UUID".into())
    })?;
    require_instance_permission(&state, &claims.sub, &id, "server:settings")
        .await?;

    if state.instances.contains_key(&iid) {
        return Err(ApiError::Conflict(
            "stop the instance before deleting".into(),
        ));
    }

    state.instance_store.delete(&iid).await?;
    activity_service::record(
        &state,
        &claims.sub,
        "instance_deleted",
        Some(&iid.to_string()),
        None,
        None,
    )
    .await?;
    state
        .broadcaster
        .send(Event::InstanceDeleted { instance_id: iid });
    Ok(Json(json!({ "ok": true })))
}
