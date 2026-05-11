# src/infrastructure — Port implementations

Concrete adapters: SQLite repos, Supabase JWT auth, PTY process spawning, Deno macro engine, Minecraft-specific HTTP clients.

## File structure

```
infrastructure/
  mod.rs          — re-exports: db, events, auth, process, minecraft, macro_engine
  events.rs       — EventBroadcaster: tokio broadcast channel wrapper for instance events
  auth/           — Supabase RS256 JWT validation
  db/             — SQLite repositories (InstanceRepo, JavaRepo, ModpackRepo)
  macro_engine/   — embedded Deno V8 runtime for JS/TS macros
  minecraft/      — jar download, installer, Java detection, Modrinth API, mrpack, server.properties
  process/        — PTY process spawning, per-instance actor, mock spawner
```

## Redirections

| Folder | AGENTS.md | Summary |
|--------|-----------|---------|
| `auth/` | `auth/AGENTS.md` | JWKS fetch + RS256 JWT validation for Supabase tokens |
| `db/` | `db/AGENTS.md` | SQLite implementations of InstanceStore, JavaStore, ModpackStore |
| `macro_engine/` | `macro_engine/AGENTS.md` | Embedded Deno V8 runtime: executor, TypeScript loader, op extensions |
| `minecraft/` | `minecraft/AGENTS.md` | Minecraft-specific infra: jar downloads, Java detection, Modrinth API, mrpack, server.properties |
| `process/` | `process/AGENTS.md` | PTY spawner, per-instance actor, mock spawner for tests |

## Gotchas

- **`EventBroadcaster`** wraps `tokio::sync::broadcast`. Receivers are created via `subscribe()`. The channel is lossy — lagged receivers drop messages silently by design.
