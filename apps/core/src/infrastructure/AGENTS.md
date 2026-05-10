# infrastructure/

Concrete implementations of ports and shared infrastructure utilities.

## Subdirectories

| Dir | What it contains |
|-----|-----------------|
| `auth/` | Supabase RS256 JWT validation via JWKS endpoint |
| `db/` | SQLite implementations of `InstanceStore` + `ModpackStore` |
| `macro_engine/` | Deno Core JS/TS runtime for user macros |
| `minecraft/` | Server JAR downloads, loader installers, mrpack, Modrinth API, Java detection |
| `process/` | PTY process spawning + per-instance lifecycle actor |

## `events.rs` — `EventBroadcaster`

Thin wrapper around `tokio::sync::broadcast::Sender<Event>` (capacity 512).

```rust
impl EventBroadcaster {
    pub fn new() -> Self;
    pub fn send(&self, event: Event);
    pub fn subscribe(&self) -> broadcast::Receiver<Event>;
}
```

Used by `instance_actor` to emit stdout lines, status changes, and creation progress.  
Used by `presentation/handlers/console.rs` (SSE + WebSocket) to subscribe.

## `mod.rs`

Re-exports `events`, `auth`, `db`, `minecraft`, `process`, `macro_engine`.

## Rules

- Implementations must satisfy the port traits in `ports/`.
- `minecraft/` may make outbound HTTP calls (Modrinth, Mojang, Fabric Meta, etc.).
- `macro_engine/` requires a dedicated `std::thread` + `tokio::task::LocalSet` per isolate.
- Never import `axum` or `presentation` types here.
- Never import `application/` services here (avoid circular deps).
