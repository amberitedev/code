# Core Architecture

### src/main.rs
Purpose: Entry point — wires config, DB, AppState, instance restore, and Axum server.
Exports: none (binary crate entry)
Theseus calls: none
Patches applied: none

### src/config.rs
Purpose: Loads runtime configuration from environment variables.
Exports: `Config` (data_dir, supabase_url, port, allowed_origin, dev_mode)
Theseus calls: none
Patches applied: none

### src/domain/mod.rs
Purpose: Re-exports domain submodules.
Exports: `instance`, `modpack`, `event`
Theseus calls: none
Patches applied: none

### src/domain/event.rs
Purpose: Defines the broadcast event enum emitted by instances and the macro engine.
Exports: `Event` (InstanceOutput, StatusChanged, MacroOutput, CreationProgress)
Theseus calls: none
Patches applied: none

### src/domain/instance.rs
Purpose: Core domain types for server instances — IDs, status lifecycle, loader variants, memory settings, persisted record.
Exports: `InstanceId`, `InstanceStatus`, `ModLoader`, `MemorySettings`, `InstanceRecord`
Theseus calls: none
Patches applied: none

### src/domain/modpack.rs
Purpose: Domain types for `.mrpack` modpack format and persisted manifest.
Exports: `PackFormat`, `PackFile`, `PackFileHashes`, `PackFileEnv`, `EnvType`, `LinkedData`, `ModpackManifest`
Theseus calls: none
Patches applied: none

### src/application/mod.rs
Purpose: Re-exports all application service submodules.
Exports: `state`, `instance_service`, `instance_status_service`, `modpack_service`, `macro_service`, `mod_service`, `log_service`, `stats_service`, `export_service`
Theseus calls: none
Patches applied: none

### src/application/state.rs
Purpose: Defines `AppState` — the shared server state injected into all handlers via `Arc<AppState>`.
Exports: `AppState`, `WsTicket`
Theseus calls: none
Patches applied: none

### src/application/instance_service.rs
Purpose: Creates new server instances — writes data dir, server.properties, DB record, and spawns background JAR download; also restores running instances on startup.
Exports: `CreateInstanceRequest`, `InstanceError`, `create_instance`, `restore_instances`, `update_port`, `get_data_dir`
Theseus calls: none
Patches applied: none

### src/application/instance_status_service.rs
Purpose: Lifecycle control for running instances — start, stop, kill, restart, send console command, set DB+broadcast status.
Exports: `start_instance`, `stop_instance`, `kill_instance`, `send_command`, `restart_instance`, `set_status`
Theseus calls: none
Patches applied: none

### src/application/mod_service.rs
Purpose: Full mod CRUD — list (filesystem + DB), add from Modrinth, upload JAR, delete, toggle enabled, update single mod, update all mods.
Exports: `ModInfo`, `UpdateAllResult`, `ModError`, `list_mods`, `add_mod`, `upload_mod`, `delete_mod`, `toggle_mod`, `update_mod`, `update_all_mods`
Theseus calls: none
Patches applied: none

### src/application/log_service.rs
Purpose: Reads server log files and crash reports from the instance data directory.
Exports: `LogEntry`, `LogError`, `list_logs`, `resolve_log`, `list_crash_reports`, `resolve_crash`
Theseus calls: none
Patches applied: none

### src/application/macro_service.rs
Purpose: Manages Deno JS/TS macros — spawn, kill, list running PIDs, list available macro files for an instance.
Exports: `MacroError`, `spawn_macro`, `kill_macro`, `list_macros`, `list_macro_files`
Theseus calls: none
Patches applied: none

### src/application/modpack_service.rs
Purpose: Installs a `.mrpack` modpack into an instance and persists the manifest; also get and remove manifest.
Exports: `ModpackError`, `install`, `get_manifest`, `remove`
Theseus calls: none
Patches applied: none

### src/application/stats_service.rs
Purpose: Collects CPU%, RAM, player count (via `list` command + event stream), and uptime for a running instance.
Exports: `StatsResponse`, `StatsError`, `get_stats`
Theseus calls: none
Patches applied: none

### src/application/export_service.rs
Purpose: Exports an instance's mod list as a `.mrpack` zip archive (Modrinth-linked mods + override JARs + config).
Exports: `ExportError`, `export_modpack`
Theseus calls: none
Patches applied: none

### src/ports/mod.rs
Purpose: Re-exports port submodules.
Exports: `instance_store`, `modpack_store`, `process_spawner`
Theseus calls: none
Patches applied: none

### src/ports/instance_store.rs
Purpose: Port trait for instance persistence; defines `InstanceStore` async trait and `StoreError`.
Exports: `StoreError`, `InstanceStore`
Theseus calls: none
Patches applied: none

### src/ports/modpack_store.rs
Purpose: Port trait for modpack manifest persistence.
Exports: `ModpackStore` (save, get_for_instance, delete_for_instance)
Theseus calls: none
Patches applied: none

### src/ports/process_spawner.rs
Purpose: Port traits for spawning and controlling child processes — `ProcessHandle` and `ProcessSpawner`.
Exports: `SpawnError`, `ProcessHandle`, `ProcessSpawner`
Theseus calls: none
Patches applied: none

### src/infrastructure/mod.rs
Purpose: Re-exports infrastructure submodules.
Exports: `db`, `events`, `auth`, `process`, `minecraft`, `macro_engine`
Theseus calls: none
Patches applied: none

### src/infrastructure/events.rs
Purpose: Thin broadcast channel wrapper for dispatching `Event` to all WebSocket/SSE subscribers.
Exports: `EventBroadcaster` (new, send, subscribe)
Theseus calls: none
Patches applied: none

### src/infrastructure/auth/mod.rs
Purpose: Re-exports `jwks` submodule.
Exports: `jwks`
Theseus calls: none
Patches applied: none

### src/infrastructure/auth/jwks.rs
Purpose: Fetches and caches Supabase JWKS, validates RS256 JWTs, returns decoded `Claims`.
Exports: `JwksCache`, `Claims`, `AuthError`
Theseus calls: none
Patches applied: none

### src/infrastructure/db/mod.rs
Purpose: Exports DB repos and provides `connect()` to create or open the SQLite pool.
Exports: `instance_repo`, `modpack_repo`, `connect`
Theseus calls: none
Patches applied: none

### src/infrastructure/db/instance_repo.rs
Purpose: SQLite implementation of `InstanceStore` using `sqlx`; maps `InstanceRow` → `InstanceRecord`.
Exports: `InstanceRepo`
Theseus calls: none
Patches applied: none

### src/infrastructure/db/modpack_repo.rs
Purpose: SQLite implementation of `ModpackStore`; upserts and fetches `ModpackManifest` rows.
Exports: `ModpackRepo`
Theseus calls: none
Patches applied: none

### src/infrastructure/macro_engine/mod.rs
Purpose: Re-exports macro engine submodules.
Exports: `executor`, `loader`, `ops`
Theseus calls: none
Patches applied: none

### src/infrastructure/macro_engine/executor.rs
Purpose: Spawns Deno JS/TS macros in isolated V8 threads, tracks them by PID, supports kill and list.
Exports: `MacroPid`, `MacroExecutor` (new, spawn_macro, kill_macro, list_pids)
Theseus calls: none
Patches applied: none

### src/infrastructure/macro_engine/loader.rs
Purpose: Deno `ModuleLoader` impl — loads `.ts`/`.js` from filesystem, transpiles TypeScript via `deno_ast`.
Exports: `TypescriptModuleLoader`
Theseus calls: none
Patches applied: none

### src/infrastructure/macro_engine/ops/mod.rs
Purpose: Re-exports Deno op extension submodules.
Exports: `prelude`, `events`, `instance_control`
Theseus calls: none
Patches applied: none

### src/infrastructure/macro_engine/ops/prelude.rs
Purpose: `amberite_prelude` Deno extension — exposes `op_get_version` op to macro JS runtime.
Exports: `amberite_prelude` (Deno extension), `op_get_version`
Theseus calls: none
Patches applied: none

### src/infrastructure/macro_engine/ops/events.rs
Purpose: `amberite_events` Deno extension — async ops for macro JS to await next instance stdout line or status change.
Exports: `EventRx`, `amberite_events` (Deno extension), `op_next_instance_output`, `op_next_state_change`
Theseus calls: none
Patches applied: none

### src/infrastructure/macro_engine/ops/instance_control.rs
Purpose: `amberite_instance_control` Deno extension — sync ops for macro JS to send commands, get status, or stop its instance.
Exports: `amberite_instance_control` (Deno extension), `op_send_command`, `op_get_status`, `op_stop_instance`
Theseus calls: none
Patches applied: none

### src/infrastructure/minecraft/mod.rs
Purpose: Re-exports all Minecraft infrastructure submodules.
Exports: `flavours`, `installer`, `modrinth_api`, `mrpack`, `server_jar`, `java`, `server_properties`
Theseus calls: none
Patches applied: none

### src/infrastructure/minecraft/flavours.rs
Purpose: Resolves server JAR download URLs for Vanilla (Mojang), Paper, and Fabric loaders.
Exports: `FlavourError`, `JarInfo`, `resolve_jar`
Theseus calls: Mojang version manifest, PaperMC API, Fabric Meta API (outbound HTTP)
Patches applied: none

### src/infrastructure/minecraft/installer.rs
Purpose: Downloads and runs Quilt/Forge/NeoForge GUI installers; detects post-install launch style and writes `launch.json`.
Exports: `InstallerError`, `LaunchStyle`, `LaunchConfig`, `write_launch_config`, `read_launch_config`, `install_with_installer`
Theseus calls: QuiltMC Maven, Forge files, NeoForge Maven, Forge promotions API (outbound HTTP)
Patches applied: none

### src/infrastructure/minecraft/java.rs
Purpose: Detects Java installations via `which`, persists them to DB, and looks up the right version for a given MC version.
Exports: `JavaInstall`, `required_java_version`, `detect_java_installations`, `sync_java_to_db`, `find_java`
Theseus calls: none
Patches applied: none

### src/infrastructure/minecraft/modrinth_api.rs
Purpose: HTTP client for Modrinth API — project lookup, version lookup, version listing, hash-based version lookup.
Exports: `ModrinthError`, `ModrinthProject`, `ModrinthVersion`, `ModrinthFile`, `ModrinthHashes`, `ModrinthClient`
Theseus calls: api.modrinth.com/v2 (outbound HTTP)
Patches applied: none

### src/infrastructure/minecraft/mrpack.rs
Purpose: Parses and installs `.mrpack` modpack files — extracts metadata, downloads server-side mods, extracts overrides.
Exports: `MrpackError`, `extract_metadata`, `install_mrpack`
Theseus calls: CDN download URLs from pack manifest (outbound HTTP)
Patches applied: none

### src/infrastructure/minecraft/server_jar.rs
Purpose: Orchestrates full server install — dispatches to `flavours::resolve_jar` or `installer::install_with_installer`, verifies SHA1, writes `launch.json`.
Exports: `ServerJarError`, `download_server_jar`
Theseus calls: none (delegates to flavours/installer)
Patches applied: none

### src/infrastructure/minecraft/server_properties.rs
Purpose: Read, write, and in-place patch `server.properties` files.
Exports: `PropertiesError`, `read_properties`, `write_properties`, `write_initial_properties`, `patch_properties`
Theseus calls: none
Patches applied: none

### src/infrastructure/process/mod.rs
Purpose: Re-exports process submodules.
Exports: `pty_spawner`, `mock_spawner`, `instance_actor`
Theseus calls: none
Patches applied: none

### src/infrastructure/process/instance_actor.rs
Purpose: Per-instance async actor — forwards stdout as events, handles start/stop/kill/command messages, updates status on exit.
Exports: `ActorCmd`, `InstanceHandle`, `spawn_actor`
Theseus calls: none
Patches applied: none

### src/infrastructure/process/pty_spawner.rs
Purpose: `ProcessSpawner` impl using `portable_pty` — opens a PTY pair, spawns process, forwards stdout lines over mpsc channel.
Exports: `PtyHandle`, `PtySpawner`
Theseus calls: none
Patches applied: none

### src/infrastructure/process/mock_spawner.rs
Purpose: In-memory fake `ProcessSpawner` for unit tests — no real process is created.
Exports: `MockSpawner`, `MockHandle`
Theseus calls: none
Patches applied: none

### src/presentation/mod.rs
Purpose: Re-exports presentation submodules.
Exports: `error`, `extractors`, `handlers`, `router`
Theseus calls: none
Patches applied: none

### src/presentation/router.rs
Purpose: Builds the full Axum router — wires all REST and WebSocket routes to handlers with CORS and tracing middleware.
Exports: `create_router`
Theseus calls: none
Patches applied: none

### src/presentation/error.rs
Purpose: Unified `ApiError` enum — maps application service errors to HTTP status codes and JSON error responses.
Exports: `ApiError` (Unauthorized, NotFound, BadRequest, Conflict, Internal, UnprocessableEntity)
Theseus calls: none
Patches applied: none

### src/presentation/extractors.rs
Purpose: `AuthUser` Axum extractor — validates Supabase Bearer JWT via `JwksCache`, returns `Claims`; dev mode bypasses auth.
Exports: `AuthUser`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/mod.rs
Purpose: Re-exports all handler submodules.
Exports: `console`, `diagnostics`, `instance_control`, `instances`, `logs`, `macros`, `modpack`, `mods`, `properties`, `setup`, `stats`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/console.rs
Purpose: WebSocket console handler (stdout stream + stdin forwarding), SSE creation-progress stream, WS ticket issuance.
Exports: `issue_ws_token`, `ws_console`, `sse_progress`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/diagnostics.rs
Purpose: Health, version, and Java installations diagnostic endpoints (no auth required).
Exports: `health`, `version`, `java_installations`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/setup.rs
Purpose: First-run pairing — validates pairing code, stores Supabase URL and owner user ID; reports paired status.
Exports: `complete_setup`, `setup_status`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/instances.rs
Purpose: CRUD for server instances — list, get, create (delegates to instance_service), delete (requires offline).
Exports: `list_instances`, `get_instance`, `create_instance`, `delete_instance`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/instance_control.rs
Purpose: Lifecycle control endpoints — start, stop, kill, restart, send console command.
Exports: `start`, `stop`, `kill`, `restart`, `send_command_handler`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/logs.rs
Purpose: Log file endpoints — list logs, serve log file (gzip-aware), list crash reports, serve crash report.
Exports: `list_logs_handler`, `read_log_handler`, `list_crash_reports_handler`, `read_crash_report_handler`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/macros.rs
Purpose: Macro endpoints — list available files and running PIDs, spawn macro by name, kill macro by PID.
Exports: `list_macros_handler`, `spawn_macro_handler`, `kill_macro_handler`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/modpack.rs
Purpose: Modpack endpoints — install mrpack via multipart upload, get manifest, remove manifest, export as `.mrpack` download.
Exports: `install_modpack`, `get_modpack`, `remove_modpack`, `export_modpack_handler`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/mods.rs
Purpose: Mod endpoints — list, add from Modrinth, upload JAR, delete, toggle enabled, update single mod, update all mods.
Exports: `list_mods_handler`, `add_mod_handler`, `upload_mod_handler`, `delete_mod_handler`, `toggle_mod_handler`, `update_mod_handler`, `update_all_handler`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/properties.rs
Purpose: Minecraft `server.properties` endpoints — read all keys and patch specific keys in-place. Queries instance `data_dir` from SQLite then delegates to `infrastructure::minecraft::server_properties`.
Exports: `get_properties_handler`, `patch_properties_handler`
Theseus calls: none
Patches applied: none

### src/presentation/handlers/stats.rs
Purpose: Resource stats endpoint — delegates to `application::stats_service::get_stats` and returns JSON.
Exports: `get_stats_handler`
Theseus calls: none
Patches applied: none














