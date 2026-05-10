# application/

Business logic orchestration. Services receive `&Arc<AppState>` and call ports or infrastructure. No HTTP types (`axum::*`) anywhere in this layer.

## Files

| File | Purpose |
|------|---------|
| `state.rs` | `AppState` — central shared state, created once in `main.rs` |
| `instance_service.rs` | `create_instance`, `restore_instances`, `get_data_dir`, `update_port` |
| `instance_status_service.rs` | `start_instance`, `stop_instance`, `kill_instance`, `restart_instance`, `send_command` |
| `mod_service.rs` | Mod CRUD: list, add (Modrinth), upload, delete, toggle, update, update-all |
| `log_service.rs` | Read logs and crash-reports from `data_dir/instances/<id>/` |
| `macro_service.rs` | `spawn_macro`, `kill_macro`, `list_macros`, `list_macro_files` |
| `modpack_service.rs` | Install `.mrpack` from Modrinth; get, remove modpack manifest |
| `export_service.rs` | Export instance mods as a `.mrpack` zip |
| `stats_service.rs` | CPU%, RAM bytes for a running instance |
| `mod.rs` | Re-exports |

## `AppState` (state.rs)

```rust
pub struct AppState {
    pub pool:                   SqlitePool,          // kept for legacy direct queries
    pub http:                   reqwest::Client,
    pub config:                 Config,
    pub instances:              DashMap<InstanceId, InstanceHandle>,
    pub broadcaster:            EventBroadcaster,
    pub macro_executor:         MacroExecutor,
    pub jwks_cache:             JwksCache,
    pub ws_tickets:             DashMap<String, WsTicket>,
    pub pairing_code:           tokio::sync::Mutex<Option<String>>,
    pub wrong_pairing_attempts: AtomicU32,           // SEC-01 lockout counter
    pub instance_store:         Arc<dyn InstanceStore>,
    pub modpack_store:          Arc<dyn ModpackStore>,
}
```

`AppState::new()` generates the 6-digit pairing code if not already paired (reads `core_config`).

## `create_instance` flow

1. Allocate `InstanceId::new()`, create `data_dir/instances/<id>/`.
2. Write `server.properties` via `write_initial_properties`.
3. Insert `InstanceRecord` (status = `offline`) via `instance_store.create()`.
4. `tokio::spawn` background JAR download — sends `CreationProgress` events. Failures are logged but do not affect the HTTP response.
5. Return `InstanceId` (HTTP 200).

**BEH-04 open bug**: returns 200, should return 201 Created.

## `restore_instances` (startup)

Called from `main.rs` via `tokio::spawn`. Resets `starting`/`stopping` → `offline`, then re-starts any `running` instances.

## Service error types

| Service | Error enum |
|---------|------------|
| `instance_service` | `InstanceError` |
| `instance_status_service` | `InstanceError` (reused) |
| `mod_service` | `ModError` |
| `log_service` | `LogError` |
| `modpack_service` | `ModpackError` |
| `export_service` | `ExportError` |
| `stats_service` | `StatsError` |
| `macro_service` | `MacroError` |

All implement `thiserror::Error`. `ApiError` in `presentation/error.rs` has `From` impls for each.

## Log path guard (`log_service.rs`)

`resolve_log` and `resolve_crash` reject filenames containing `..`, `/`, or `\` (SEC-03).  
Extension whitelist: logs → `.log`, `.log.gz`; crash-reports → `.txt`.

## Rules

- Services receive `&Arc<AppState>` — never own state.
- Background tasks use `tokio::spawn`.
- No `axum` types here.
- `update_port` in `instance_service.rs` still uses raw `sqlx::query` (ARCH-01 open).
