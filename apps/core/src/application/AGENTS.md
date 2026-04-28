# application/

Business logic orchestration. Services call ports and infrastructure; they never touch HTTP or SQL directly.

## Files

| File | Purpose |
|------|---------|
| `state.rs` | `AppState` — shared server state injected into all handlers |
| `instance_service.rs` | Create instance (spawns background install task) |
| `instance_status_service.rs` | start / stop / kill / restart / send_command |
| `mod_service.rs` | Mod CRUD — list, add (Modrinth), upload, delete, toggle, update, update-all |
| `log_service.rs` | Read logs + crash-reports from data dir |
| `macro_service.rs` | spawn_macro / kill_macro / list_macros / list_macro_files |
| `modpack_service.rs` | Install mrpack from Modrinth, get, remove |
| `export_service.rs` | Export instance mods as `.mrpack` zip |
| `stats_service.rs` | CPU%, RAM bytes, player count (via broadcaster) |
| `mod.rs` | Re-exports |

## `AppState`

```rust
pub struct AppState {
    pub pool:           SqlitePool,
    pub http:           reqwest::Client,
    pub config:         CoreConfig,          // data_dir, bind_addr, jwks_url
    pub instances:      DashMap<InstanceId, InstanceHandle>,
    pub broadcaster:    EventBroadcaster,
    pub macro_executor: MacroExecutor,
    pub jwks_cache:     Arc<JwksCache>,
    pub ws_tickets:     DashMap<String, InstanceId>,
    pub pairing_code:   Arc<Mutex<Option<String>>>,
    pub instance_store: Arc<dyn InstanceStore + Send + Sync>,
    pub modpack_store:  Arc<dyn ModpackStore + Send + Sync>,
}
```

## Error types per service

| Service | Error enum |
|---------|-----------|
| instance_service | `InstanceError` |
| instance_status_service | `StatusError` |
| mod_service | `ModError` |
| log_service | `LogError` |
| modpack_service | `ModpackError` |
| export_service | `ExportError` |
| stats_service | `StatsError` |
| macro_service | `MacroError` |

All implement `thiserror::Error`. `ApiError` in `presentation/error.rs` has `From` impls for each.

## Rules

- Services receive `&Arc<AppState>` — never own state.
- Background tasks (instance creation, installs) are spawned with `tokio::spawn`.
- No HTTP handler types (`axum::*`) in this layer.
- Macro paths resolve to `data_dir/instances/<id>/macros/<name>.ts` or `.js`.
