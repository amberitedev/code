//! Archon HTTP contracts backed by Core's existing management services.
//! Mounted only in loopback development mode; account pairing is separate work.

#[path = "backup_queue.rs"]
mod backup_queue;

use super::models::*;
use crate::{
    application::{
        backup_service, mod_service, modpack_service,
        state::{AppState, WsTicket},
    },
    domain::{
        event::{Event, FsOperationKind},
        instance::{
            InstanceId, InstanceInstallStatus, InstanceRecord, InstanceStatus,
            MemorySettings, ModLoader,
        },
    },
    infrastructure::minecraft::server_properties,
    presentation::{
        authz::{
            can_access_instance, require_core_manager,
            require_instance_permission,
        },
        error::ApiError,
        extractors::AuthUser,
        handlers::{
            backups, instance_control, instances, modpack, mods, properties,
        },
        instance_path::resolve_instance_path,
    },
};
use axum::{
    body::Body,
    extract::{Path, Query, State},
    http::{header, StatusCode},
    response::Response,
    routing::{delete, get, post},
    Json, Router,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

type ApiResult = Result<Json<Value>, ApiError>;

pub fn router() -> Router<Arc<AppState>> {
    Router::new()
        .merge(backup_queue::router())
        .route(
            "/servers/:id/modrinth/v0/backups/:bid/download",
            get(download_backup),
        )
        .route("/local/servers", post(create_server))
        .route("/modrinth/v0/servers", get(list_v0))
        .route("/modrinth/v0/servers/:id", get(get_v0))
        .route("/modrinth/v0/servers/:id/power", post(power))
        .route("/modrinth/v0/servers/:id/name", post(rename))
        .route("/modrinth/v0/servers/:id/fs", get(filesystem_auth))
        .route("/modrinth/v0/servers/:id/ws", get(websocket_auth))
        .route("/modrinth/v0/servers/:id/reinstall", post(reinstall))
        .route(
            "/modrinth/v0/servers/:id/allocations",
            get(allocations).post(unsupported),
        )
        .route(
            "/modrinth/v0/servers/:id/allocations/:port",
            axum::routing::put(unsupported).delete(unsupported),
        )
        .route("/modrinth/v0/servers/:id/subdomain", post(unsupported))
        .route(
            "/modrinth/v0/subdomains/:subdomain/isavailable",
            get(unsupported),
        )
        .route(
            "/modrinth/v0/servers/:id/startup",
            get(startup_v0).post(patch_startup_v0),
        )
        .route(
            "/modrinth/v0/servers/:id/notices/:notice/dismiss",
            post(unsupported),
        )
        .route("/v1/servers", get(list_v1))
        .route("/v1/servers/:id", get(get_v1))
        .route("/v1/servers/:id/flows/intro", delete(end_intro))
        .route("/v1/servers/:id/worlds/:wid/onboard", post(unsupported))
        .route(
            "/v1/servers/:id/worlds/:wid/options/startup",
            get(startup_v1).patch(patch_startup_v1),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/properties",
            get(get_properties).patch(patch_properties),
        )
        .route("/v1/servers/:id/worlds/:wid/content", post(install_content))
        .route("/v1/servers/:id/worlds/:wid/content/repair", post(repair))
        .route(
            "/v1/servers/:id/worlds/:wid/content/unlink-modpack",
            post(unlink_modpack),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons",
            get(get_addons).post(add_addon),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/install-many",
            post(add_addons),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/delete",
            post(delete_addon),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/delete-many",
            post(delete_addons),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/disable",
            post(disable_addon),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/enable",
            post(enable_addon),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/disable-many",
            post(disable_addons),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/enable-many",
            post(enable_addons),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/update",
            get(unsupported).post(update_addon),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/addons/update-many",
            post(update_addons),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/content/modpack/update",
            get(unsupported).post(unsupported),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/content/update-game-version",
            get(unsupported).post(unsupported),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/backups",
            get(list_backups).post(create_backup),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/backups/:bid",
            get(get_backup).patch(rename_backup).delete(delete_backup),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/backups/:bid/restore",
            post(restore_backup),
        )
        .route(
            "/v1/servers/:id/worlds/:wid/backups/:bid/retry",
            post(unsupported),
        )
}

async fn unsupported(_user: AuthUser) -> ApiResult {
    Err(ApiError::UnprocessableEntity(
        "This operation is not implemented by the local Core".into(),
    ))
}

fn host(state: &AppState) -> String {
    reqwest::Url::parse(&state.config.public_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_else(|| "127.0.0.1".into())
}

fn world(id: String, wid: String) -> Result<String, ApiError> {
    if id != wid {
        return Err(ApiError::NotFound(
            "world not found on this server".into(),
        ));
    }
    Ok(id)
}

async fn record(
    state: &Arc<AppState>,
    user: &AuthUser,
    id: &str,
    permission: &str,
) -> Result<InstanceRecord, ApiError> {
    let record = resolve_instance_path(state, id).await?;
    require_instance_permission(
        state,
        &user.0.sub,
        &record.id.to_string(),
        permission,
    )
    .await?;
    Ok(record)
}

async fn visible(
    state: &Arc<AppState>,
    user: &AuthUser,
) -> Result<Vec<InstanceRecord>, ApiError> {
    let mut visible = Vec::new();
    for r in state.instance_store.list().await? {
        if can_access_instance(
            state,
            &user.0.sub,
            &r.id.to_string(),
            "server:view",
        )
        .await
        {
            visible.push(r);
        }
    }
    Ok(visible)
}

async fn v0(
    state: &Arc<AppState>,
    r: &InstanceRecord,
) -> Result<Value, ApiError> {
    let pack = modpack_service::get_manifest(state, &r.id.to_string()).await?;
    let upstream = pack.and_then(|p| Some(json!({"kind": "modpack", "project_id": p.modrinth_project_id?, "version_id": p.modrinth_version_id?}))).unwrap_or(Value::Null);
    let count = backup_service::list_backups(state, &r.id.to_string())
        .await?
        .len();
    // All Hosting routes are restricted to the local development owner.
    let node_url = format!(
        "{}/hosting/servers/{}",
        state.config.public_url.trim_end_matches('/'),
        r.id
    );
    Ok(server_v0(
        r,
        "local-noauth-owner",
        -32768,
        count,
        &host(state),
        upstream,
        &node_url,
    ))
}

async fn v1(
    state: &Arc<AppState>,
    r: &InstanceRecord,
) -> Result<Value, ApiError> {
    let rows = backup_service::list_backups(state, &r.id.to_string()).await?;
    Ok(server_v1(
        r,
        &host(state),
        rows.iter().map(backup).collect(),
    ))
}

async fn list_v0(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Query(q): Query<Pagination>,
) -> ApiResult {
    let records = visible(&state, &user).await?;
    let count = records.len();
    let limit = q.limit.unwrap_or(100).clamp(1, 1000);
    let offset = q.offset.unwrap_or(0);
    let mut servers = Vec::new();
    for r in records.into_iter().skip(offset).take(limit) {
        servers.push(v0(&state, &r).await?);
    }
    Ok(Json(
        json!({"servers": servers, "users": {"local-noauth-owner": {"id": "local-noauth-owner", "username": "Local developer", "avatar_url": null}},
        "pagination": {"current_page": offset / limit + 1, "page_size": limit, "total_pages": count.div_ceil(limit), "total_items": count}}),
    ))
}

async fn get_v0(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult {
    let r = record(&state, &user, &id, "server:view").await?;
    Ok(Json(v0(&state, &r).await?))
}

async fn list_v1(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
) -> ApiResult {
    let mut servers = Vec::new();
    for r in visible(&state, &user).await? {
        servers.push(v1(&state, &r).await?);
    }
    Ok(Json(json!(servers)))
}

async fn get_v1(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult {
    let r = record(&state, &user, &id, "server:view").await?;
    Ok(Json(v1(&state, &r).await?))
}

async fn create_server(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Json(body): Json<CreateServer>,
) -> Result<(StatusCode, Json<Value>), ApiError> {
    require_core_manager(&state, &user.0.sub).await?;
    let name = body.name.trim();
    if name.is_empty() || name.len() > 100 {
        return Err(ApiError::BadRequest(
            "name must contain 1-100 characters".into(),
        ));
    }
    let memory = body.memory_mb.unwrap_or(4096);
    if memory < 512 {
        return Err(ApiError::BadRequest(
            "memory_mb must be at least 512".into(),
        ));
    }
    let existing = state.instance_store.list().await?;
    let port = match body.port {
        Some(0) => {
            return Err(ApiError::BadRequest("port cannot be zero".into()))
        }
        Some(port) if existing.iter().any(|r| r.port == port) => {
            return Err(ApiError::Conflict("port is already assigned".into()))
        }
        Some(port) => port,
        None => (25565..=65535)
            .find(|port| !existing.iter().any(|r| r.port == *port))
            .ok_or_else(|| ApiError::Conflict("no unassigned ports".into()))?,
    };
    let id = InstanceId::new();
    let directory =
        state.config.data_dir.join("instances").join(id.to_string());
    tokio::fs::create_dir_all(&directory)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    server_properties::write_initial_properties(&directory, port)
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?;
    let now = chrono::Utc::now();
    // An empty game version means onboarding has not selected server content yet.
    let r = InstanceRecord {
        id: id.clone(),
        path: id.to_string(),
        name: name.into(),
        game_version: String::new(),
        loader: ModLoader::Vanilla,
        loader_version: None,
        port,
        memory: MemorySettings {
            min_mb: 512,
            max_mb: memory,
        },
        java_version: None,
        jvm_args: None,
        server_args: None,
        install_status: InstanceInstallStatus::Installing,
        status: InstanceStatus::Offline,
        data_dir: directory.display().to_string(),
        installation_id: None,
        total_uptime_seconds: 0,
        created_at: now,
        updated_at: now,
    };
    state.instance_store.create(&r).await?;
    state
        .broadcaster
        .send(Event::InstanceCreated { instance: r });
    Ok((StatusCode::CREATED, Json(json!({"id": id}))))
}

async fn power(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Power>,
) -> ApiResult {
    match body.action {
        PowerAction::Start => {
            instance_control::start(user, Path(id), State(state)).await
        }
        PowerAction::Stop => {
            instance_control::stop(user, Path(id), State(state)).await
        }
        PowerAction::Restart => {
            instance_control::restart(user, Path(id), State(state)).await
        }
        PowerAction::Kill => {
            instance_control::kill(user, Path(id), State(state)).await
        }
    }
}

async fn rename(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Name>,
) -> ApiResult {
    instances::patch_instance(
        user,
        Path(id),
        State(state),
        Json(instances::PatchBody {
            name: Some(body.name),
            java_version: None,
            memory: None,
            jvm_args: None,
            server_args: None,
        }),
    )
    .await
}

async fn end_intro(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let r = record(&state, &user, &id, "server:settings").await?;
    if r.game_version.is_empty() {
        return Err(ApiError::Conflict(
            "Choose server content before finishing setup".into(),
        ));
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn allocations(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult {
    let r = record(&state, &user, &id, "server:view").await?;
    Ok(Json(json!([{"port": r.port, "name": "Minecraft"}])))
}

async fn filesystem_auth(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult {
    let r = record(&state, &user, &id, "server:files").await?;
    Ok(Json(
        json!({"url": format!("{}/hosting/servers/{}/modrinth/v0/fs", state.config.public_url.trim_end_matches('/'), r.id), "token": ""}),
    ))
}

async fn websocket_auth(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult {
    let r = record(&state, &user, &id, "server:view").await?;
    let token = uuid::Uuid::new_v4().to_string();
    state
        .ws_tickets
        .retain(|_, ticket| ticket.expires_at > Instant::now());
    state.ws_tickets.insert(
        token.clone(),
        WsTicket {
            user_id: format!("dev:{}", r.id),
            expires_at: Instant::now() + Duration::from_secs(60),
        },
    );
    let base = state
        .config
        .public_url
        .trim_end_matches('/')
        .replacen("http", "ws", 1);
    Ok(Json(
        json!({"url": format!("{base}/hosting/servers/{}/ws", r.id), "token": token}),
    ))
}

async fn reinstall(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Query(q): Query<ReinstallQuery>,
    Json(body): Json<Reinstall>,
) -> ApiResult {
    if q.hard {
        return Err(ApiError::UnprocessableEntity(
            "Hard reset is not implemented by the local Core".into(),
        ));
    }
    match body {
        Reinstall::Loader {
            loader: value,
            loader_version,
            game_version,
        } => {
            instance_control::change_version_handler(
                user,
                Path(id),
                State(state),
                Json(instance_control::ChangeVersionBody {
                    game_version,
                    loader: Some(loader(&value)?),
                    loader_version: Some(loader_version),
                }),
            )
            .await
        }
        Reinstall::Modpack {
            project_id,
            version_id,
        } => {
            let version_id = version_id.ok_or_else(|| {
                ApiError::BadRequest(
                    "version_id is required for a modpack installation".into(),
                )
            })?;
            install_modrinth_pack(&user, &state, &id, &project_id, &version_id)
                .await?;
            Ok(Json(json!({})))
        }
    }
}

async fn startup_v0(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
) -> ApiResult {
    let Json(startup) =
        instances::get_startup(user, Path(id), State(state)).await?;
    Ok(Json(
        json!({"invocation": startup["effective_command"], "original_invocation": startup["default_command"],
        "jdk_version": startup["java_version"].as_i64().map(|v| format!("lts{v}")), "jdk_build": null}),
    ))
}

async fn startup_v1(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> ApiResult {
    let Json(startup) =
        instances::get_startup(user, Path(world(id, wid)?), State(state))
            .await?;
    Ok(Json(
        json!({"java_version": startup["java_version"], "jre_vendor": null,
        "original_invocation": startup["default_command"], "startup_command": startup["effective_command"]}),
    ))
}

async fn patch_startup_v0(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(id): Path<String>,
    Json(body): Json<Value>,
) -> ApiResult {
    if body.get("invocation").is_some_and(|v| !v.is_null())
        || body.get("jdk_build").is_some_and(|v| !v.is_null())
    {
        return Err(ApiError::UnprocessableEntity(
            "Custom startup commands and JDK vendors are not supported by Core"
                .into(),
        ));
    }
    let version = body
        .get("jdk_version")
        .map(|v| {
            if v.is_null() {
                Ok(None)
            } else {
                v.as_str()
                    .and_then(|s| s.strip_prefix("lts"))
                    .and_then(|s| s.parse::<i64>().ok())
                    .map(Some)
                    .ok_or_else(|| {
                        ApiError::BadRequest("invalid JDK version".into())
                    })
            }
        })
        .transpose()?;
    patch_java(user, state, id, version).await
}

async fn patch_startup_v1(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<Value>,
) -> ApiResult {
    let id = world(id, wid)?;
    if body.get("startup_command").is_some_and(|v| !v.is_null())
        || body.get("jre_vendor").is_some_and(|v| !v.is_null())
    {
        return Err(ApiError::UnprocessableEntity(
            "Custom startup commands and JDK vendors are not supported by Core"
                .into(),
        ));
    }
    let version = body
        .get("java_version")
        .map(|v| {
            if v.is_null() {
                Ok(None)
            } else {
                v.as_i64().filter(|v| *v > 0).map(Some).ok_or_else(|| {
                    ApiError::BadRequest("invalid Java version".into())
                })
            }
        })
        .transpose()?;
    patch_java(user, state, id, version).await
}

async fn patch_java(
    user: AuthUser,
    state: Arc<AppState>,
    id: String,
    version: Option<Option<i64>>,
) -> ApiResult {
    instances::patch_instance(
        user,
        Path(id),
        State(state),
        Json(instances::PatchBody {
            name: None,
            java_version: version,
            memory: None,
            jvm_args: None,
            server_args: None,
        }),
    )
    .await
}

async fn get_properties(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> ApiResult {
    let Json(value) = properties::get_properties_handler(
        user,
        Path(world(id, wid)?),
        State(state),
    )
    .await?;
    let mut known = serde_json::Map::new();
    let mut custom = serde_json::Map::new();
    if let Some(props) = value["properties"].as_object() {
        for (key, value) in props {
            let normalized = key.replace('-', "_");
            if KNOWN_PROPERTIES.contains(&normalized.as_str()) {
                known.insert(normalized, value.clone());
            } else {
                custom.insert(key.clone(), value.clone());
            }
        }
    }
    Ok(Json(json!({"known": known, "custom": custom})))
}

async fn patch_properties(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<PropertiesPatch>,
) -> ApiResult {
    let id = world(id, wid)?;
    apply_properties(&user, &state, &id, body).await?;
    get_properties(user, State(state), Path((id.clone(), id))).await
}

async fn apply_properties(
    user: &AuthUser,
    state: &Arc<AppState>,
    id: &str,
    body: PropertiesPatch,
) -> Result<(), ApiError> {
    // Null means remove a key in Archon. Core's patch handler cannot remove keys,
    // so reject the operation instead of silently changing it to an empty value.
    if body
        .known
        .values()
        .chain(body.custom.values())
        .any(Option::is_none)
    {
        return Err(ApiError::UnprocessableEntity("Removing properties is not supported through this endpoint; edit server.properties in Files".into()));
    }
    let mut updates: HashMap<String, String> = body
        .custom
        .into_iter()
        .map(|(k, v)| (k, v.unwrap()))
        .collect();
    for (key, value) in body.known {
        if !KNOWN_PROPERTIES.contains(&key.as_str()) {
            return Err(ApiError::BadRequest(format!(
                "unknown property {key}"
            )));
        }
        updates.insert(key.replace('_', "-"), value.unwrap());
    }
    let _ = properties::patch_properties_handler(
        AuthUser(user.0.clone()),
        Path(id.into()),
        State(state.clone()),
        Json(updates),
    )
    .await?;
    Ok(())
}

async fn install_content(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<InstallContent>,
) -> ApiResult {
    let id = world(id, wid)?;
    let r = record(&state, &user, &id, "server:content").await?;
    let props = match body {
        InstallContent::Bare {
            loader: value,
            version,
            game_version,
            soft_override,
            properties,
        } => {
            if !soft_override && !r.game_version.is_empty() {
                return Err(ApiError::UnprocessableEntity(
                    "Hard content reset is not implemented by Core".into(),
                ));
            }
            let target = loader(&value)?;
            let loader_version =
                if target == ModLoader::Vanilla || version.is_empty() {
                    None
                } else {
                    Some(version.clone())
                };
            let game_version = game_version
                .or_else(|| {
                    if target == ModLoader::Vanilla {
                        Some(version.clone())
                    } else {
                        None
                    }
                })
                .filter(|v| !v.trim().is_empty())
                .ok_or_else(|| {
                    ApiError::BadRequest("game_version is required".into())
                })?;
            let _ = instance_control::change_version_handler(
                AuthUser(user.0.clone()),
                Path(id.clone()),
                State(state.clone()),
                Json(instance_control::ChangeVersionBody {
                    game_version: Some(game_version),
                    loader: Some(target),
                    loader_version: Some(loader_version),
                }),
            )
            .await?;
            properties
        }
        InstallContent::Modpack {
            spec,
            soft_override,
            properties,
        } => {
            if !soft_override && !r.game_version.is_empty() {
                return Err(ApiError::UnprocessableEntity(
                    "Hard content reset is not implemented by Core".into(),
                ));
            }
            match spec {
                ModpackSpec::Modrinth {
                    project_id,
                    version_id,
                } => {
                    install_modrinth_pack(
                        &user,
                        &state,
                        &id,
                        &project_id,
                        &version_id,
                    )
                    .await?;
                }
                ModpackSpec::LocalFile { filename } => {
                    return Err(ApiError::UnprocessableEntity(format!(
                        "Install {filename} using the mrpack upload endpoint"
                    )))
                }
            }
            properties
        }
    };
    if let Some(mut props) = props {
        // Setup sends null for unset defaults such as a random world seed.
        // This is an initial install, not an instruction to remove properties.
        props.known.retain(|_, value| value.is_some());
        props.custom.retain(|_, value| value.is_some());
        apply_properties(&user, &state, &id, props).await?;
    }
    Ok(Json(json!({})))
}

async fn repair(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> ApiResult {
    instance_control::repair(user, Path(world(id, wid)?), State(state)).await
}

async fn install_modrinth_pack(
    user: &AuthUser,
    state: &Arc<AppState>,
    id: &str,
    project: &str,
    version: &str,
) -> Result<(), ApiError> {
    let r = record(state, user, id, "server:content").await?;
    if state.instances.contains_key(&r.id) {
        return Err(ApiError::Conflict(
            "Stop the server before installing a modpack".into(),
        ));
    }
    let manifest = modpack_service::install_modrinth_version(
        state,
        &r.id.to_string(),
        project,
        version,
    )
    .await?;
    if manifest.game_version.is_empty() {
        return Err(ApiError::BadRequest(
            "Modpack has no Minecraft version".into(),
        ));
    }
    crate::application::instance_service::change_version(
        state,
        &r.id,
        Some(manifest.game_version),
        Some(loader(&manifest.loader)?),
        Some(manifest.loader_version),
    )
    .await?;
    Ok(())
}

async fn unlink_modpack(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> ApiResult {
    modpack::remove_modpack(user, Path(world(id, wid)?), State(state)).await
}

async fn get_addons(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> ApiResult {
    let id = world(id, wid)?;
    let r = record(&state, &user, &id, "server:content").await?;
    let mut addons = Vec::new();
    for m in mod_service::list_mods(&state, &id).await? {
        let path = std::path::Path::new(&r.data_dir)
            .join("mods")
            .join(&m.filename);
        let size = tokio::fs::metadata(path)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
        addons.push(addon(&m, size));
    }
    let pack = modpack_service::get_manifest(&state, &id).await?;
    let mut result = json!({"modloader": r.loader.to_string(), "modloader_version": r.loader_version,
        "game_version": r.game_version, "modpack": pack.as_ref().map(super::models::modpack), "addons": addons});
    match r.install_status {
        InstanceInstallStatus::Installing if !r.game_version.is_empty() => {
            result["installing"] = json!("loader")
        }
        InstanceInstallStatus::Failed => {
            result["error"] =
                json!({"message": "Server content installation failed"})
        }
        _ => {}
    }
    Ok(Json(result))
}

fn require_mod(kind: Option<&str>) -> Result<(), ApiError> {
    if kind.is_some_and(|kind| kind != "mod") {
        return Err(ApiError::UnprocessableEntity("Only mod addons are currently managed by Core; use Files for other content".into()));
    }
    Ok(())
}

async fn add_addon(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<AddAddon>,
) -> ApiResult {
    require_mod(body.kind.as_deref())?;
    mods::add_mod_handler(
        user,
        Path(world(id, wid)?),
        State(state),
        Json(mods::AddModBody {
            project_id: Some(body.project_id),
            version_id: body.version_id,
        }),
    )
    .await
}

async fn add_addons(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(pair): Path<(String, String)>,
    Json(items): Json<Vec<AddAddon>>,
) -> ApiResult {
    for item in &items {
        require_mod(item.kind.as_deref())?;
    }
    for item in items {
        let _ = add_addon(
            AuthUser(user.0.clone()),
            State(state.clone()),
            Path(pair.clone()),
            Json(item),
        )
        .await?;
    }
    Ok(Json(json!({})))
}

async fn delete_addon(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<AddonFile>,
) -> ApiResult {
    require_mod(Some(&body.kind))?;
    mods::delete_mod_handler(
        user,
        Path((world(id, wid)?, body.filename)),
        State(state),
    )
    .await
}

async fn delete_addons(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(pair): Path<(String, String)>,
    Json(body): Json<AddonFiles>,
) -> ApiResult {
    for item in &body.items {
        require_mod(Some(&item.kind))?;
    }
    for item in body.items {
        let _ = delete_addon(
            AuthUser(user.0.clone()),
            State(state.clone()),
            Path(pair.clone()),
            Json(item),
        )
        .await?;
    }
    Ok(Json(json!({})))
}

async fn toggle(
    user: AuthUser,
    state: Arc<AppState>,
    pair: (String, String),
    body: AddonFile,
    enabled: bool,
) -> ApiResult {
    require_mod(Some(&body.kind))?;
    mods::toggle_mod_handler(
        user,
        Path((world(pair.0, pair.1)?, body.filename)),
        State(state),
        Json(mods::ToggleBody { enabled }),
    )
    .await
}

async fn disable_addon(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(pair): Path<(String, String)>,
    Json(body): Json<AddonFile>,
) -> ApiResult {
    toggle(user, state, pair, body, false).await
}
async fn enable_addon(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(pair): Path<(String, String)>,
    Json(body): Json<AddonFile>,
) -> ApiResult {
    toggle(user, state, pair, body, true).await
}

async fn toggle_many(
    user: AuthUser,
    state: Arc<AppState>,
    pair: (String, String),
    body: AddonFiles,
    enabled: bool,
) -> ApiResult {
    for item in &body.items {
        require_mod(Some(&item.kind))?;
    }
    for item in body.items {
        let _ = toggle(
            AuthUser(user.0.clone()),
            state.clone(),
            pair.clone(),
            item,
            enabled,
        )
        .await?;
    }
    Ok(Json(json!({})))
}
async fn disable_addons(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(pair): Path<(String, String)>,
    Json(body): Json<AddonFiles>,
) -> ApiResult {
    toggle_many(user, state, pair, body, false).await
}
async fn enable_addons(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(pair): Path<(String, String)>,
    Json(body): Json<AddonFiles>,
) -> ApiResult {
    toggle_many(user, state, pair, body, true).await
}

async fn update_addon(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    Json(body): Json<UpdateAddon>,
) -> ApiResult {
    if body.version_id.is_some() {
        return Err(ApiError::UnprocessableEntity(
            "Selecting an addon update version is not yet supported by Core"
                .into(),
        ));
    }
    mods::update_mod_handler(
        user,
        Path((world(id, wid)?, body.filename)),
        State(state),
    )
    .await
}

async fn update_addons(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path(pair): Path<(String, String)>,
    Json(body): Json<UpdateAddons>,
) -> ApiResult {
    if body.addons.iter().any(|a| a.version_id.is_some()) {
        return Err(ApiError::UnprocessableEntity(
            "Selecting an addon update version is not yet supported by Core"
                .into(),
        ));
    }
    for item in body.addons {
        let _ = update_addon(
            AuthUser(user.0.clone()),
            State(state.clone()),
            Path(pair.clone()),
            Json(item),
        )
        .await?;
    }
    Ok(Json(json!({})))
}

async fn backup_rows(
    user: &AuthUser,
    state: &Arc<AppState>,
    id: &str,
) -> Result<Vec<Value>, ApiError> {
    let r = record(state, user, id, "server:backups").await?;
    Ok(backup_service::list_backups(state, &r.id.to_string())
        .await?
        .iter()
        .map(backup)
        .collect())
}

async fn list_backups(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
) -> ApiResult {
    Ok(Json(json!(
        backup_rows(&user, &state, &world(id, wid)?).await?
    )))
}

async fn get_backup(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid, bid)): Path<(String, String, String)>,
) -> ApiResult {
    let rows = backup_rows(&user, &state, &world(id, wid)?).await?;
    rows.into_iter()
        .find(|b| b["id"].as_str() == Some(&bid))
        .map(Json)
        .ok_or_else(|| ApiError::NotFound("backup not found".into()))
}

async fn create_backup(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid)): Path<(String, String)>,
    body: Json<backups::CreateBody>,
) -> ApiResult {
    let id = world(id, wid)?;
    let Json(created) = backups::create_handler(
        user,
        Path(id.clone()),
        State(state.clone()),
        body,
    )
    .await?;
    notify_changed(&state, &id, false).await?;
    Ok(Json(json!({"id": created["id"]})))
}

async fn delete_backup(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid, bid)): Path<(String, String, String)>,
) -> ApiResult {
    let id = world(id, wid)?;
    let result = backups::delete_handler(
        user,
        Path((id.clone(), bid)),
        State(state.clone()),
    )
    .await?;
    notify_changed(&state, &id, false).await?;
    Ok(result)
}

async fn rename_backup(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid, bid)): Path<(String, String, String)>,
    body: Json<backups::RenameBody>,
) -> ApiResult {
    let id = world(id, wid)?;
    let result = backups::rename_handler(
        user,
        Path((id.clone(), bid)),
        State(state.clone()),
        body,
    )
    .await?;
    notify_changed(&state, &id, false).await?;
    Ok(result)
}

async fn restore_backup(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, wid, bid)): Path<(String, String, String)>,
) -> ApiResult {
    let id = world(id, wid)?;
    let result = backups::restore_handler(
        user,
        Path((id.clone(), bid)),
        State(state.clone()),
    )
    .await?;
    notify_changed(&state, &id, true).await?;
    Ok(result)
}

async fn notify_changed(
    state: &Arc<AppState>,
    id: &str,
    files: bool,
) -> Result<(), ApiError> {
    let r = resolve_instance_path(state, id).await?;
    if files {
        state.broadcaster.send(Event::FsChanged {
            instance_id: r.id.clone(),
            operation: FsOperationKind::Unzip,
            path: "/".into(),
        });
    }
    state
        .broadcaster
        .send(Event::InstanceUpdated { instance: r });
    Ok(())
}

async fn download_backup(
    user: AuthUser,
    State(state): State<Arc<AppState>>,
    Path((id, bid)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    let r = record(&state, &user, &id, "server:backups").await?;
    // Resolve against this server's database rows before deriving any disk path.
    let rows = backup_service::list_backups(&state, &r.id.to_string()).await?;
    let row = rows
        .into_iter()
        .find(|b| b.id == bid)
        .ok_or_else(|| ApiError::NotFound("backup not found".into()))?;
    let backup_id = uuid::Uuid::parse_str(&row.id)
        .map_err(|_| ApiError::BadRequest("invalid backup ID".into()))?;
    let path = state
        .config
        .data_dir
        .join("backups")
        .join(r.id.to_string())
        .join(format!("{backup_id}.zip"));
    let file = tokio::fs::File::open(path).await.map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ApiError::NotFound("backup archive not found".into())
        } else {
            ApiError::Internal(e.to_string())
        }
    })?;
    let size = file
        .metadata()
        .await
        .map_err(|e| ApiError::Internal(e.to_string()))?
        .len();
    Response::builder()
        .header(header::CONTENT_TYPE, "application/zip")
        .header(header::CONTENT_LENGTH, size)
        .header(
            header::CONTENT_DISPOSITION,
            format!("attachment; filename=\"{backup_id}.zip\""),
        )
        .body(Body::from_stream(tokio_util::io::ReaderStream::new(file)))
        .map_err(|e| ApiError::Internal(e.to_string()))
}
