# src/presentation — HTTP handlers, routing, auth extraction, error mapping

Axum HTTP layer. Translates between HTTP request/response and application service calls. Contains no business logic.

## File structure

```
presentation/
  mod.rs          — re-exports: error, extractors, handlers, router
  router.rs       — create_router(): assembles all routes with CORS and tracing middleware
  error.rs        — ApiError enum with From impls for every service error type
  extractors.rs   — AuthUser Axum extractor (validates Bearer JWT or dev bypass)
  handlers/
    mod.rs              — re-exports all handlers
    console.rs          — WS console (stdout stream + stdin), SSE creation progress, WS ticket issue
    diagnostics.rs      — GET /health, POST /connection/handshake, GET /version, GET /java (no auth)
    setup.rs            — POST /setup (pairing), GET /setup/status (no auth)
    instances.rs        — CRUD: list, get, create, delete
    instance_control.rs — start, stop, kill, restart, send command
    logs.rs             — list/read log files and crash reports
    macros.rs           — macro routes; execution currently disabled by application service
    modpack.rs          — install (multipart), get manifest, remove, export .mrpack
    mods.rs             — list, add from Modrinth, upload JAR, delete, toggle, update, update-all
    fs.rs               — instance filesystem operations plus one-time download-token streaming
    properties.rs       — GET/PATCH server.properties
    social.rs           — Core metadata, members, bans, and sync-profile scaffolding
    stats.rs            — GET resource stats (CPU, RAM, player count, uptime)
```

## Route table

All routes are in `router.rs`. Grouped by auth requirement:

**No auth** — `AuthUser` extractor not used:
- `GET /health`, `POST /connection/handshake`, `GET /version`, `GET /java`
- `POST /setup`, `GET /setup/status` (`POST` accepts either a 6-digit `code` or local `local_setup_secret`)
- `GET /instances/:id/console?ticket=<uuid>` — ticket IS the credential (validated inside handler)

**Requires `AuthUser`** (Bearer JWT or dev bypass):
- Everything else, including `POST /ws-token`
- `GET/PATCH /core`, `/core/members`, `/core/bans`, and `/sync/profiles*` are owner-authenticated scaffolding for social/friend-group management; fine-grained member authorization is not enforced yet.

## extractors.rs — AuthUser

`AuthUser` is an Axum extractor that validates a Bearer JWT using the `JwksCache`. If `config.dev_mode` is true, it skips JWT validation entirely and returns synthetic `Claims { sub: "dev-owner" }`. This means every route using `AuthUser` is open in dev mode — no token header needed.

`bearer_token()` does an exact case-sensitive prefix match on `"Bearer "` (capital B, trailing space). A lowercase `"bearer "` header returns `None`, which becomes a 401.

The JWKS URL comes from `state.jwks_url().await` — a DB query against `core_config.auth_jwks_url`. If Core is not paired (no row), `jwks_url()` returns `None` and the extractor returns 401. Auth is structurally impossible before pairing.

## console.rs — WebSocket flow

WebSocket auth uses a two-step ticket flow:
1. Client calls `POST /ws-token` with a valid Bearer JWT → receives a UUID ticket (60s TTL).
2. Client connects `GET /instances/:id/console?ticket=<uuid>` — no Bearer header needed here.

`validate_ticket` uses `DashMap::remove` — the ticket is consumed on first use (single-use). An expired ticket still returns 401 even if it was never used.

The WebSocket handler (`ws_handler`) filters the broadcast stream to only `InstanceOutput` events for the matching instance ID. `StatusChanged` and other event types are silently dropped. Incoming text messages from the WebSocket are forwarded as console commands via `send_command`.

`sse_progress` filters for `CreationProgress` events for a specific instance and emits them as SSE data frames with a keep-alive ping. This endpoint is the only way to track JAR download progress after `POST /instances`.

## error.rs — ApiError

`ApiError` has `From` impls for every service error type. All JSON responses have the shape `{ "error": "..." }`.

Service-to-HTTP status mappings worth knowing:
- `InstanceError::AlreadyRunning` → 409
- `InstanceError::NotRunning` → 409
- `InstanceError::ActorDead` → 503
- `ModError::ClientOnly` → 422 (semantic rejection — the mod is client-only)

`ApiError::TooManyRequests` is only produced by `setup.rs` after 5 wrong pairing code attempts.

## Gotchas

- **CORS is restricted outside dev mode** — `router.rs` uses `ALLOWED_ORIGIN` in production and permissive CORS only in dev mode or when `ALLOWED_ORIGIN=*`.
- **WebSocket endpoint bypasses `AuthUser`** — the ticket in the query string IS the auth. Don't add `AuthUser` to `ws_console` — it would require a JWT header on a WebSocket upgrade, which browsers can't provide.
- **`GET /fs/file/:key` bypasses `AuthUser`** — the one-time download key IS the credential. Tokens are removed on use and expired tokens are drained by the background GC task.
- **`setup.rs` uses raw `sqlx::query`** — the pairing handler writes directly to `state.pool` rather than going through a store port. This is intentional — `core_config` is not an entity with a port.
- **Macro routes are disabled stubs** — they remain registered for API compatibility but return service unavailable until the future out-of-process plugin system is implemented.
