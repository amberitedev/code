# domain/

Pure business logic. **No I/O, no async, no external crates beyond `serde` / `uuid` / `chrono`.**

## Files

| File | Key types |
|------|-----------|
| `instance.rs` | `InstanceId(Uuid)`, `InstanceRecord`, `InstanceStatus`, `MemorySettings`, `ModLoader` |
| `modpack.rs` | `ModpackManifest`, `PackFormat`, `PackFile`, `EnvType` |
| `event.rs` | `Event` — the broadcast payload enum |
| `mod.rs` | Re-exports |

## Key Types

### `InstanceStatus`
`offline | starting | running | stopping | crashed | installing`

Serialises/parses via `Display` / `FromStr` (lowercase string in DB and API).

### `ModLoader`
`vanilla | paper | fabric | quilt | forge | neoforge`

### `MemorySettings`
`{ min_mb: u32, max_mb: u32 }` — JVM heap flags.

### `Event`
```rust
enum Event {
    InstanceOutput  { instance_id, line },
    StatusChanged   { instance_id, status },
    MacroOutput     { pid, line },
    CreationProgress{ instance_id, stage, pct },
}
```
Broadcast via `tokio::sync::broadcast` from `infrastructure::events::EventBroadcaster`.

### `PackFormat` (mrpack index)
Top-level JSON of a `.mrpack` file — `game`, `name`, `version_id`, `summary`, `files`, `dependencies`.

## Rules

- No `async` anywhere in this layer.
- No `sqlx`, `axum`, `reqwest`, or any network/IO crate.
- `InstanceId` is a newtype over `uuid::Uuid` — always serialise as lowercase hyphenated string.
