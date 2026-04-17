# Lodestone Core — src/

Source code for Lodestone Core backend. Clean Architecture with four layers.

## Context Loading

| Subdirectory | AGENTS.md | What it covers |
|--------------|-----------|----------------|
| `domain/` | `domain/AGENTS.md` | Entities, typestates, ports |
| `application/` | `application/AGENTS.md` | Services, actors, orchestration |
| `infrastructure/` | `infrastructure/AGENTS.md` | Repositories, Deno, processes |
| `presentation/` | `presentation/AGENTS.md` | Routes, handlers, WebSocket |

Read subdirectory `AGENTS.md` files on-demand for layer-specific details.

## Directory Structure

```
src/
├── main.rs                    # Entry point: wires layers, starts Axum server on :16662
│
├── domain/                    # Pure business logic — zero external dependencies
│   ├── mod.rs                 # Module exports
│   ├── auth.rs                # UserId, User, Role, UserPermission types
│   ├── config.rs              # SettingManifest, ConfigurableValue for UI
│   ├── flavours.rs            # Minecraft server variants (Vanilla, Paper, Fabric)
│   ├── instances.rs           # GameInstance with typestate (Stopped/Running)
│   └── ports.rs               # Trait definitions (Repository, ProcessManager, ScriptRuntime)
│
├── application/               # Orchestrates domain logic using infrastructure
│   ├── mod.rs                 # Module exports
│   ├── registry.rs            # ServiceRegistry for dependency injection
│   ├── auth_service.rs        # Login, registration, PASETO token generation
│   ├── instance_service.rs    # Manages active instance actors via DashMap
│   ├── instance_actor.rs      # Isolated actor per instance with command channel
│   └── macro_engine.rs        # Executes JavaScript macros via Deno
│
├── infrastructure/            # Concrete implementations of domain ports
│   ├── mod.rs                 # Module exports
│   ├── sqlite_repo.rs         # SQLite implementations of repositories
│   ├── process_spawner.rs     # Spawns Java processes via tokio::process
│   ├── deno_runtime.rs        # Deno JsRuntime on dedicated thread
│   ├── server_properties.rs   # Minecraft server.properties definitions
│   ├── server_properties_macro.rs  # Macro generates property boilerplate
│   └── networking.rs          # UPnP, playit.gg tunneling, port management
│
└── presentation/              # HTTP API layer (Axum)
    ├── mod.rs                 # Module exports
    ├── router.rs              # Axum router assembly, route definitions
    ├── error.rs               # ApiError types, HTTP response mapping
    ├── extractors.rs          # Custom Axum extractors (Auth, InstanceService)
    └── handlers/
        ├── mod.rs             # Handler module exports
        ├── auth_api.rs        # POST /login, POST /setup
        ├── instance_api.rs    # POST /instances/:id/{start,stop,kill,command}
        ├── diagnostics_api.rs # GET /health, GET /stats
        └── websockets.rs      # GET /instances/:id/console (WS)
```

## API Structure

- **Auth** (`handlers/auth_api.rs`) — Login, initial setup
- **Instances** (`handlers/instance_api.rs`) — Start, stop, kill, send command
- **Diagnostics** (`handlers/diagnostics_api.rs`) — Health check, system stats
- **WebSocket** (`handlers/websockets.rs`) — Real-time console streaming

Router assembles routes in `presentation/router.rs`. Error mapping in `presentation/error.rs`.

## Key Types

| Type | Location | Purpose |
|------|----------|---------|
| `InstanceId` | `domain/instances.rs` | Newtype wrapper for UUID |
| `GameInstance<State>` | `domain/instances.rs` | Typestate entity (Stopped/Running) |
| `UserId` | `domain/auth.rs` | Newtype wrapper for UUID |
| `ServiceRegistry` | `application/registry.rs` | Dependency injection |
| `InstanceActor` | `application/instance_actor.rs` | Actor per instance |
| `SqliteRepo` | `infrastructure/sqlite_repo.rs` | Repository implementation |
| `ApiError` | `presentation/error.rs` | HTTP error response |

## Testing

Tests are colocated: `#[cfg(test)] mod tests { ... }` at end of files.

```bash
# Run all tests from core/
cargo test

# Run with output
cargo test -- --nocapture

# Run specific test
cargo test -- <test_name>

# Run module tests
cargo test -- instance_service
```

Mock implementations in `infrastructure/`:
- `MockProcessSpawner` — fake process spawning
- `MockDenoRuntime` — fake script execution

## Coding Standards

See `core/AGENTS.md` for global rules:
- Max ~200 lines per file
- One struct/enum per file
- Explicit imports (no `*`)
- Colocated tests
- Auth via middleware/extractors

## Database

Tables (see `migrations/001_init.sql`):
- `users` — id, username, hashed_password, role
- `instances` — id, name, created_at
- `paseto_key` — id, key (binary)
- `events` — id, instance_id, event_type, data, created_at

SQLite with SQLx migrations (auto-applied on startup).

## Environment

`.env` in data directory:
```
LODESTONE_PATH=/path/to/data
PLAYIT_SECRET_KEY=optional
```
