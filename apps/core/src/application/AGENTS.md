# src/application — Services and shared state

Orchestration layer: coordinates domain types with port interfaces. Most direct I/O is delegated to `ports` and `infrastructure`; `pairing_service` is the current exception because it performs one startup Convex registration through the shared HTTP client. All services receive `&Arc<AppState>`.

## File structure

```
application/
  mod.rs                    — re-exports all service submodules
  state.rs                  — AppState definition and constructors
  instance_service.rs       — create instance, restore on startup, get data_dir
  instance_status_service.rs — start, stop, kill, restart, send command, set_status
  mod_service.rs            — full mod CRUD (list, add from Modrinth, upload, delete, toggle, update)
  modpack_service.rs        — install .mrpack, get/remove manifest
  log_service.rs            — list + read log files and crash reports from data_dir
  macro_service.rs          — disabled macro route backing service
  pairing_service.rs        — register unpaired remote Cores with Convex
  social_service.rs         — Core metadata, local member permission records, bans, sync profile list
  social_lookup_service.rs  — Small shared SQLite lookups for social services
  social_models.rs          — Request/response structs and SocialError shared by social services
  social_sync_service.rs    — Sync profile registration plus snapshot/event persistence; diff/apply deferred
  stats_service.rs          — CPU%, RAM, player count, uptime for a running instance
  export_service.rs         — export instance mod list as a .mrpack archive
```

## state.rs — AppState

`AppState` is always behind `Arc<AppState>` — it is never passed by value. The `Arc` is constructed inside `AppState::new()` and returned.

Key fields:

| Field | Type | Purpose |
|-------|------|---------|
| `pool` | `SqlitePool` | Raw SQLite pool — kept for direct queries that bypass the port traits (e.g., `mods` table, `core_config`) |
| `http` | `reqwest::Client` | Shared HTTP client (user-agent `amberite-core/0.1`) used by all outbound requests |
| `config` | `Config` | Runtime configuration loaded from env vars |
| `core_id` | `String` | Stable Core UUID loaded from `core_identity`, generated once on first startup |
| `instances` | `DashMap<InstanceId, InstanceHandle>` | Live running instances — present only while the actor task is alive |
| `broadcaster` | `EventBroadcaster` | Broadcast channel for instance output + status events |
| `jwks_cache` | `JwksCache` | Cached JWKS keys for RS256 JWT validation |
| `ws_tickets` | `DashMap<String, WsTicket>` | Short-lived UUID tokens for WebSocket auth |
| `fs_download_tokens` | `DashMap<String, FsDownloadToken>` | Short-lived one-time file download tokens, drained by `gc_fs_download_tokens` |
| `pairing_code` | `Mutex<Option<String>>` | Set at startup if unpaired; cleared after successful pairing |
| `local_setup_secret` | `Mutex<Option<String>>` | One-time secret written to `.setup_secret` for app-launched local pairing |
| `wrong_pairing_attempts` | `AtomicU32` | Pairing lockout counter (max 5) |
| `instance_store` | `Arc<dyn InstanceStore>` | SQLite-backed in production |
| `java_store` | `Arc<dyn JavaStore>` | SQLite-backed in production |
| `modpack_store` | `Arc<dyn ModpackStore>` | SQLite-backed in production |
| `spawner` | `Arc<dyn AnySpawner>` | `StdSpawner` in production, `MockSpawner` in tests |

`AppState::new()` calls `AppState::new_with_spawner()` with `StdSpawner`. Tests use `new_with_spawner(MockSpawner)` to avoid spawning real JVMs. `AMBERITE_DEV` defaults to false even in debug builds; tests set `dev_mode` explicitly.

`jwks_url()` is an async method that queries `auth_jwks_url` from `core_config` in the DB — it is NOT derived from `Config.convex_url`.

`pairing_service::register_pairing_core` runs once at startup for unpaired Cores when `CONVEX_URL` is set. It registers the current 6-digit code, stable `core_id`, optional `AMBERITE_PUBLIC_URL`, and bind metadata with Convex.

Owner/member identity currently comes from Convex Auth `users._id`, not `users.amberiteUserId`. Core `AuthUser` compares JWT `sub` to `core_config.owner_user_id`, and Convex social functions store the same ID string in memberships.

## instance_service.rs

`create_instance`: writes `data_dir/{uuid}/`, creates initial `server.properties`, inserts the DB record, then spawns a background tokio task for the JAR download. The function returns the new `InstanceId` immediately — the download happens asynchronously. Use `GET /instances/:id/progress` (SSE) to track it. If the download fails, the DB record still exists and the instance will be in `Offline` status with no JAR.

`restore_instances`: runs at startup as a separate `tokio::spawn` task. Detects Java installations and syncs to DB, resets any instances stuck in `starting` or `stopping` to `offline` (unclean-shutdown recovery), then re-starts all instances that had `Running` status.

## instance_status_service.rs

`start_instance`: reads `launch.json` from the instance `data_dir` to determine launch style. If `launch.json` is absent (legacy instances), it falls back to `server.jar` in the data dir.

Launch styles from `launch.json`:
- `Jar { jar }` — `java -Xms... -Xmx... -jar {jar} --nogui` (Vanilla, Paper, Fabric, Quilt post-install)
- `ArgsFile { args }` — `java -Xms... -Xmx... @{args}` (Forge 1.17+ uses an args file from libraries/)

`restart_instance`: sends `GracefulStop`, then polls `state.instances.contains_key(id)` until the actor removes itself (clean stop signal). 30-second timeout.

`set_status` is `pub(crate)` — called by both this service and `instance_actor`. It updates the DB and broadcasts a `StatusChanged` event.

## mod_service.rs

`list_mods` scans the `mods/` subdirectory of the instance data_dir and cross-references against the `mods` DB table. The `tracked` flag on `ModInfo` means the file has a DB row; untacked JARs dropped directly into the folder still appear in the list.

`sanitize_filename` rejects filenames containing `..`, `/`, or `\`. Called before any filesystem operation on a mod filename (SEC-05 path traversal guard).

## Other services

`stats_service::get_stats`: gets CPU/RAM from `sysinfo` filtered by PID (from `InstanceHandle.pid`), and player count by sending the `"list"` command and listening to the event stream for the response line. Uptime is derived from `InstanceHandle.started_at`.

`export_service::export_modpack`: builds a `.mrpack` ZIP with `modrinth.index.json` listing Modrinth-linked mods and override JARs for untracked files, plus the contents of the instance's `config/` directory as overrides.

`macro_service`: macro execution is intentionally disabled. Routes return a service-unavailable error until the future out-of-process plugin system exists; do not re-add embedded Deno here.

`social_sync_service`: stores synchronized profile snapshots and a planned sync event, but intentionally does not resolve mod diffs or apply files yet. Continue from `publish_snapshot` when the diff/apply algorithm is designed.

`social_service`: local Core members are permission records only. Friend-group membership discovery/invites live in Convex; the Core tables mirror local enforcement data and do not currently authorize non-owner requests. `/setup` seeds the paired `owner_user_id` into `core_members` as the immutable `owner` permission record.

## Gotchas

- **`AppState.instances` presence ≠ `Running` status**: An instance can be in `Starting` or `Stopping` and still have a handle in the map. Absence from the map means the actor task has fully exited.
- **Background JAR download failures are silent in the HTTP response**: `create_instance` returns 201 before the download starts. Errors log via `tracing::error!` but don't surface to the caller.
- **`mod_service` bypasses the port abstraction**: several functions use `state.pool` directly (raw `sqlx::query`) rather than going through `instance_store`. This is intentional — the mods table is not part of the `InstanceStore` port.
