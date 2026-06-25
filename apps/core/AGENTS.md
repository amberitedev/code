# apps/core — Copal

Copal is a Rust/Axum server that manages Minecraft server instances and exposes them to the desktop app.

Core is authoritative for Core membership, roles, permissions, bans, invites, instance access, and audit logs. Convex receives only a minimal authenticated projection containing Core routing/link state and active member user IDs.

## Start Here

Do not scan, glob, or broad-search the Core tree for orientation. Context is pregenerated in these AGENTS files.

Pick the matching area below, read that AGENTS.md, then read only the source files that are directly relevant to the change. Use search only to find a known symbol, route, table, or type after the relevant area has been selected.

## Main Areas

- `src/application/` — Core workflows and business behavior.
- `src/infrastructure/` — concrete adapters: SQLite, process spawning, auth, events, Minecraft install/download code.
- `src/infrastructure/db/` — SQLite connection and repository implementations.
- `src/presentation/` — Axum routes, handlers, extractors, HTTP errors.
- `src/api/` — typed message, envelope, relay, and distribution system.
- `migrations/` — SQLite schema migrations.
- `tests/` — integration tests and fixtures.

## Routing

| Change | Read |
| ------ | ---- |
| HTTP route, request, response, status code | `src/presentation/AGENTS.md` |
| Core behavior or workflow | `src/application/AGENTS.md` |
| Database query, repository, schema | `src/infrastructure/db/AGENTS.md`, `migrations/AGENTS.md` |
| Relay, message, envelope, distribution | `src/api/AGENTS.md` |
| Process start, stop, console lifecycle | `src/infrastructure/AGENTS.md` |
| Minecraft install, Java, Modrinth, mrpack | `src/infrastructure/AGENTS.md` |
| Tests | `tests/AGENTS.md` |

## Internal Dependencies

- `presentation` calls into `application` and `api`; it should not own durable behavior.
- `application` coordinates domain data, ports, infrastructure helpers, and `AppState`.
- `ports` define traits used by application services.
- `infrastructure` implements ports and performs concrete I/O.
- `infrastructure/db` owns reusable SQLite repository implementations.
- `api` owns message vocabulary and relay/distribution concepts; HTTP exposure stays in `presentation`.
- `migrations` define the database shape that DB repositories and raw SQL expect.
- `tests` exercise Core through HTTP with test fixtures.

## External Dependencies

- Axum/Tower — HTTP server, routing, middleware, WebSocket/SSE handling.
- SQLx/SQLite — persistence and migrations.
- Tokio — async runtime and background tasks.
- Reqwest — outbound HTTP for Modrinth, auth/JWKS, Convex pairing, and other remote APIs.
- Rustls/jsonwebtoken — JWT validation.
- DashMap/broadcast channels — shared runtime maps and event fan-out.
- Minecraft/Modrinth services — server metadata, project/version lookup, mrpack downloads.
- Tauri/app frontend clients — consume Core HTTP, WebSocket, and relay APIs.

## Core Constraints

- Push minimal projection snapshots to Convex after setup and membership changes. Projection sync failures should log a warning and must not roll back the local SQLite mutation.
- Keep files around 200 lines. Ask before going substantially beyond that.
- Run Core commands from `apps/core/`.
- Do not run dev/build commands unless explicitly asked.
