# domain/

Pure business logic. **No I/O, no async, no external crates beyond `serde`, `uuid`, `chrono`.**

## Files

| File | Key types |
|------|-----------|
| `instance.rs` | `InstanceId`, `InstanceRecord`, `InstanceStatus`, `ModLoader`, `MemorySettings` |
| `modpack.rs` | `PackFormat`, `PackFile`, `PackFileHashes`, `PackFileEnv`, `EnvType`, `ModpackManifest`, `LinkedData` |
| `event.rs` | `Event` — broadcast payload enum |
| `mod.rs` | Re-exports |

## `InstanceId`

Newtype over `uuid::Uuid`. Implements `Display`, `FromStr`, `Hash`, `Eq`.  
Always serialized as lowercase hyphenated UUID string.

## `InstanceStatus`

```rust
enum InstanceStatus { Offline, Starting, Running, Stopping, Crashed }
```

Serialises/parses via `Display`/`FromStr` (lowercase). Stored as TEXT in SQLite.

## `ModLoader`

```rust
enum ModLoader { Vanilla, Paper, Fabric, Forge, NeoForge, Quilt }
```

Serialises/parses via `Display`/`FromStr` (lowercase).

## `MemorySettings`

```rust
struct MemorySettings { min_mb: u32, max_mb: u32 }
// Default: min 512, max 4096
```

## `InstanceRecord`

Persisted server record. All fields optional where the DB column is nullable.

```rust
struct InstanceRecord {
    id: InstanceId, name: String, game_version: String,
    loader: ModLoader, loader_version: Option<String>,
    port: u16, memory: MemorySettings, java_version: Option<i64>,
    status: InstanceStatus, data_dir: String,
    created_at: DateTime<Utc>, updated_at: DateTime<Utc>,
}
```

## `Event`

Broadcast via `tokio::sync::broadcast` from `infrastructure::events::EventBroadcaster`.

```rust
enum Event {
    InstanceOutput   { instance_id: InstanceId, line: String },
    StatusChanged    { instance_id: InstanceId, status: InstanceStatus },
    MacroOutput      { instance_id: InstanceId, macro_pid: u64, line: String },
    CreationProgress { instance_id: InstanceId, progress: f32, message: String },
}
```

Tagged JSON (`"type": "instance_output"` etc.) via `#[serde(tag = "type", rename_all = "snake_case")]`.

## `PackFormat` (mrpack)

Top-level of a `.mrpack` index file (`modrinth.index.json`).  
`camelCase` JSON keys. `files: Vec<PackFile>`, `dependencies: HashMap<String, String>`.

## `EnvType`

```rust
enum EnvType { Required, Optional, Unsupported }
```

`kebab-case` in JSON. Used to filter server-side-only files during mrpack install.

## Rules

- No `async` anywhere in this layer.
- No `sqlx`, `axum`, `reqwest`, or any network/IO crate.
- Unit tests live as `#[cfg(test)] mod tests` inside each file.
