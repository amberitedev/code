# infrastructure/

Concrete implementations of ports and shared infrastructure utilities.

## Subdirectories

| Dir | What it contains |
|-----|-----------------|
| `auth/` | JWT validation against Supabase JWKS endpoint |
| `db/` | SQLite implementations of `InstanceStore` + `ModpackStore` |
| `macro_engine/` | Deno Core JS/TS runtime for user macros |
| `minecraft/` | Minecraft server downloads, installers, mrpack, Modrinth API |
| `process/` | PTY process spawning and the per-instance actor |

## `events.rs`

`EventBroadcaster` — thin wrapper around `tokio::sync::broadcast::Sender<Event>`.

```rust
impl EventBroadcaster {
    pub fn send(&self, event: Event);
    pub fn subscribe(&self) -> broadcast::Receiver<Event>;
}
```

Used by instance actor to emit stdout lines, status changes, and creation progress.

## Rules

- Implementations must satisfy the port traits in `ports/`.
- `minecraft/` may make outbound HTTP calls (Modrinth, Mojang, Fabric Meta, etc.).
- `macro_engine/` requires a single-threaded Tokio `LocalSet` per isolate.
- Never import `axum` or `presentation` types here.
