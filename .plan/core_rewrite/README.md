# Amberite Core — Rewrite Plan

Takes Amberite Core from "partially working" to a production-grade Minecraft server
management backend. Fix all blockers first, then implement features in any order.

## Plan Files

| File | What it covers |
|------|----------------|
| `features.md` | Complete feature inventory — every feature with full detail |
| `01-blockers.md` | 6 bugs blocking compile + runtime — fix before anything else |
| `02-loaders.md` | All 6 server loaders: Vanilla, Fabric, Quilt, Paper, Forge, NeoForge |
| `03-mod-management.md` | Full mod library layer — the biggest new feature |
| `04-logs-properties.md` | Log history API + server properties read/write |
| `05-modpack-misc.md` | Modpack improvements, restart, stats, deferred TODOs |

---

## Architecture: What Are Repos?

The Core has a two-layer database pattern:

**Ports (interfaces):** `ports/instance_store.rs` and `ports/modpack_store.rs` define what
DB operations exist as Rust traits — "save instance", "list instances", etc. These are the
contracts every DB layer must fulfill.

**Repos (implementations):** `infrastructure/db/instance_repo.rs` and
`infrastructure/db/modpack_repo.rs` are the actual SQLite implementations. `InstanceRepo`
writes `INSERT INTO instances ...`, `ModpackRepo` writes `INSERT INTO modpacks ...`, etc.

**The current bug:** `AppState` — the shared context every request handler can see — holds
only `pool: SqlitePool`. It doesn't include the repos. Services bypass them entirely and
write raw SQL inline. The repos are fully correct code; they are just never instantiated.

**The fix:** Add `instance_store: Arc<dyn InstanceStore + Send + Sync>` and
`modpack_store: Arc<dyn ModpackStore + Send + Sync>` to `AppState`. Create `InstanceRepo`
and `ModpackRepo` at startup in `main.rs`. Update all services to call
`state.instance_store.*` / `state.modpack_store.*` instead of raw SQL.

---

## Modrinth Environment System

Every mod on Modrinth has two flags — `client_side` and `server_side` — each set to
`"required"`, `"optional"`, or `"unsupported"`. These determine where the mod must run.

| client_side  | server_side  | Meaning                                  |
|--------------|--------------|------------------------------------------|
| required     | unsupported  | Client-only — will crash or be ignored on server |
| unsupported  | required     | Server-only — no client install needed   |
| required     | required     | Must be on both client and server        |
| optional     | optional     | Works anywhere, required nowhere         |
| required     | optional     | Client required, server can have it      |
| optional     | required     | Server required, client can have it      |

**Default filter on mod search:** Hide mods where `server_side = "unsupported"`.
This is on by default. Pass `?include_client_only=true` to see everything.

**Install guard:** The Core must reject `POST /mods` requests where
`server_side = "unsupported"` with HTTP 400 and a clear message to the user.

---

## Critical Gotchas

### ❌ DON'T run migration 002 as-is
`001` creates `instances`. `002` does `CREATE TABLE IF NOT EXISTS instances` — silently
skipped. Extended columns (game_version, loader, port, etc.) are never added. Fix with
migration `003` using `ALTER TABLE instances ADD COLUMN` for each missing column.

### ❌ DON'T write raw SQL in services
The repos exist and are correct. Use `state.instance_store` and `state.modpack_store`.
Writing more inline SQL makes everything harder and inconsistent.

### ❌ DON'T exceed 200 lines per file
`instance_service.rs` is already 223 lines — it must be split. Split on logical
responsibility, not just line count.

### ❌ DON'T use std::fs inside async handlers
Use `tokio::fs` for all file I/O. Wrap anything sync in `tokio::task::spawn_blocking`.
Applies to log reading, mod scanning, JAR writing, and properties parsing.

### ❌ DON'T use sleep() in the restart endpoint
Poll actual process state until stopped (timeout ~30s). `sleep(3)` is fragile and
fails on slow servers.

### ❌ DON'T skip the server_side check before installing mods
Always fetch project metadata from Modrinth and check before downloading. A client-only
mod silently installed on a server causes confusing failures.

### ✅ DO use the existing ModrinthClient
`infrastructure/minecraft/modrinth_api.rs` is fully implemented against Modrinth v2 API.
All Modrinth HTTP calls should go through it — no new raw reqwest calls.

### ✅ DO write server.properties on instance creation
Call `write_initial_properties()` in `create_instance`. Without it every new server fails
to start: EULA not accepted, port undefined.

### ✅ DO use the sysinfo crate for CPU/RAM stats
The app runs on Windows via Tauri. Never read `/proc` directly — use `sysinfo` for
cross-platform process stats.

### ✅ DO add TODO comments for deferred features
Exact TODO text and file locations are listed in `05-modpack-misc.md`.
