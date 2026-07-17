# apps/core — Copal

Copal is Amberite's Rust/Axum server manager. It manages local Minecraft server instances and exposes authenticated REST, SSE, and console-WebSocket APIs to the desktop app.

## Ownership

Core owns local instances, files, processes, console data, roles, access, and Core-local events. Convex owns durable identity, pairing, friendships, groups, blocks, and social authorization. The Cloudflare realtime Worker owns desktop online presence.

## Parts

| Area | Purpose |
| --- | --- |
| `src/presentation/` | Axum routes, contracts, authentication, authorization, and protocol handlers |
| `src/application/` | Workflows and the shared `AppState` |
| `src/domain/` | Core data types and typed events |
| `src/ports/` | Storage and process interfaces |
| `src/infrastructure/` | SQLite, process actors, auth, events, Minecraft installation, Java, Modrinth, and file I/O |
| `migrations/` | SQLite schema history embedded by SQLx |

## Useful facts

1. `src/presentation/router.rs` is the public API map; handlers normally delegate behavior to application services.
2. SQLite stores durable instance state, while `AppState.instances` stores live process actors during running and transitional states.
3. Process control is actor-based: start creates a handle, and stop, kill, and command operations communicate through it.
4. Minecraft installation writes launch metadata that later start operations consume instead of rebuilding launch behavior.
5. Public Core contracts are mirrored in `packages/amberite-api`; durable social changes belong in Convex, and Core-local changes use the existing SSE or console stream rather than a generic relay.

## Local commands

From the repository root, inspect or apply an idempotent Core setup:

```bash
pnpm core:setup -- list
pnpm core:setup -- offline-instance
```

From `apps/core/`, run the focused checks:

```bash
cargo check
cargo run -- check
```

When a migration changes, `cargo clean -p copal` makes SQLx re-embed it.
