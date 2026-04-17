# Domain Layer

## What This Is

Pure business logic with typestate pattern and dependency inversion ports — zero external dependencies.

## Files

| File | Purpose |
|------|---------|
| `mod.rs` | Module exports |
| `instances.rs` | GameInstance typestate (Stopped/Running) |
| `ports.rs` | Dependency inversion traits |
| `auth.rs` | User, Role, UserPermission entities |
| `flavours.rs` | Minecraft server variants (Vanilla, Paper, Fabric) |
| `config.rs` | SettingManifest, ConfigurableValue types |

## Core Pattern: Typestate

`GameInstance<State>` encodes state in the type system:

```rust
impl GameInstance<Stopped> {
    pub async fn start(self, manager: &dyn OSProcessManager) 
        -> Result<GameInstance<Running>, DomainError>
}

impl GameInstance<Running> {
    pub async fn stop(self, graceful: bool) 
        -> Result<GameInstance<Stopped>, DomainError>
    pub fn send_command(&self, cmd: &str) -> Result<(), DomainError>
}
```

**Why:** Compiler rejects `stop()` on stopped or `start()` on running.

## File Details

### `instances.rs`
| Type | Purpose |
|------|---------|
| `InstanceId` | Newtype for `Uuid` |
| `Stopped` / `Running` | State markers |
| `ProcessHandle` | OS process trait |
| `GameInstance<State>` | Typestate entity |
| `DomainError` | Error enum |

### `ports.rs`
Traits for dependency inversion. Application uses these; infrastructure implements:

| Trait | Methods |
|-------|---------|
| `InstanceRepository` | get, save, list, delete |
| `UserRepository` | get, get_by_username, create, update, list |
| `OSProcessManager` | spawn, kill |
| `ScriptRuntime` | execute |
| `ConfigManager` | read_properties, write_properties |

### `auth.rs`
| Type | Purpose |
|------|---------|
| `UserId` | Newtype for `Uuid` |
| `User` | Entity with hashed password |
| `Role` | Admin or Standard(UserPermission) |
| `UserPermission` | Per-instance access (HashSet<InstanceId>) |

### `flavours.rs`
`Flavour` trait for server variants:
- `Vanilla` — Mojang Piston API
- `Paper` — PaperMC API v2
- `Fabric` — Fabric Meta API

### `config.rs`
| Type | Purpose |
|------|---------|
| `ConfigurableValue` | Enum: String, Boolean, Integer, Enum, Number |
| `SettingManifest` | UI metadata: id, name, description, value, default |

## Error Handling

`DomainError` variants: NotFound, AlreadyRunning, NotRunning, SpawnFailed, Database.

Conversions: `From<ProcessSpawnerError>`, `From<DenoRuntimeError>`.

## Testing

Pure unit tests (no external deps). Use mock ports:
- `MockProcessSpawner` — fake ProcessHandle
- `MockDenoRuntime` — no-op script execution
