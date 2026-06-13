# src/application

The application layer owns Core behavior. It coordinates domain data, database ports, process spawning, filesystem work, HTTP clients, and infrastructure helpers.

Handlers in `presentation` should usually call services here instead of implementing workflows directly.

## Mental Model

`AppState` is the shared runtime object. It holds the SQLite pool, config, HTTP client, live instance map, broadcaster, auth/cache state, setup state, and port implementations.

Instance lifecycle has two related states:

- SQLite stores durable instance records.
- `AppState.instances` stores live process actors while they are running or transitioning.

Creating an instance can return before installation work finishes. Long-running install/download work may continue in the background while the DB record already exists.

Some features use port traits and repositories. Newer or feature-specific tables may still use `state.pool` directly when no repository abstraction exists yet.

## File Relationships

- `state.rs` wires config, DB pool, ports, runtime maps, auth state, and shared clients into `AppState`.
- `instance_service.rs` owns durable instance creation, deletion, restore, and data directory setup.
- `instance_status_service.rs` owns start/stop/restart/kill/command workflows and talks to process actors through state.
- `mod_service.rs`, `modpack_service.rs`, and `export_service.rs` combine DB rows, instance files, Modrinth data, and archive logic.
- `log_service.rs` and `stats_service.rs` read runtime or instance filesystem state for API responses.
- `pairing_service.rs` handles startup registration for unpaired remote Cores.
- `access_service.rs`, `player_service.rs`, `activity_service.rs`, and `social_models.rs` are the access/member/activity model.
- `task_service.rs` and `task_scheduler.rs` own scheduled/background task behavior.
- `rcon_service.rs` owns RCON interactions.

## Related Areas

- HTTP endpoints for these services live in `src/presentation/handlers/`.
- Durable DB repositories live in `src/infrastructure/db/`.
- Process spawning and actors live in `src/infrastructure/process/`.
- Minecraft install/file helpers live in `src/infrastructure/minecraft/`.
- Message relay/distribution behavior lives in `src/api/`.
