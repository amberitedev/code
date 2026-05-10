# presentation/

Axum HTTP layer — routes, extractors, and error mapping. No business logic here.

## Files

| File | Purpose |
|------|---------|
| `router.rs` | `create_router(state)` — wires all 40+ routes |
| `error.rs` | `ApiError` — unified error type, `IntoResponse`, `From` impls |
| `extractors.rs` | `AuthUser` — validates Supabase JWT, injects `Claims` |
| `handlers/` | One file per feature area (see `handlers/AGENTS.md`) |
| `mod.rs` | Re-exports |

## Route map

| Method | Path | Auth |
|--------|------|------|
| GET | `/health`, `/version`, `/java` | none |
| GET | `/setup/status` | none |
| POST | `/setup` | none (pairing code is the credential) |
| POST | `/ws-token` | JWT |
| GET | `/instances` | JWT |
| POST | `/instances` | JWT |
| GET | `/instances/:id` | JWT |
| DELETE | `/instances/:id` | JWT |
| POST | `/instances/:id/start\|stop\|kill\|restart` | JWT |
| POST | `/instances/:id/command` | JWT |
| GET | `/instances/:id/console` (WebSocket) | WS ticket |
| GET | `/instances/:id/progress` (SSE) | JWT |
| GET/POST/DELETE | `/instances/:id/modpack` | JWT |
| GET | `/instances/:id/modpack/export` | JWT |
| GET/POST | `/instances/:id/macros` | JWT |
| DELETE | `/instances/:id/macros/:pid` | JWT |
| GET/POST | `/instances/:id/mods` | JWT |
| POST | `/instances/:id/mods/upload` | JWT |
| POST | `/instances/:id/mods/update-all` | JWT |
| DELETE/PATCH | `/instances/:id/mods/:filename` | JWT |
| PUT | `/instances/:id/mods/:filename/update` | JWT |
| GET | `/instances/:id/logs` | JWT |
| GET | `/instances/:id/logs/:filename` | JWT |
| GET | `/instances/:id/crash-reports` | JWT |
| GET | `/instances/:id/crash-reports/:filename` | JWT |
| GET/PATCH | `/instances/:id/properties` | JWT |
| GET | `/instances/:id/stats` | JWT |

CORS: `CorsLayer::permissive()` (all origins). No global body size limit (QUAL open).

## `ApiError`

```rust
pub enum ApiError {
    Unauthorized(String),    // 401
    NotFound(String),        // 404
    BadRequest(String),      // 400
    Conflict(String),        // 409
    Internal(String),        // 500
    UnprocessableEntity(String), // 422
    TooManyRequests(String), // 429 (SEC-01)
}
```

`IntoResponse` → `(StatusCode, Json({ "error": "..." }))`.  
`From` impls: `InstanceError`, `ModpackError`, `MacroError`, `ModError`, `LogError`, `StatsError`, `ExportError`, `sqlx::Error`.

## `AuthUser` extractor (`extractors.rs`)

1. Dev mode (`config.dev_mode = true`) → returns synthetic `Claims { sub: "dev-owner" }`.
2. Prod: extracts `Authorization: Bearer <token>` (case-sensitive `"Bearer "` prefix).
3. Calls `AppState::jwks_url()` → `None` if not paired → 401 "Core not paired with Supabase".
4. Calls `JwksCache::validate(token, jwks_url)` → 401 on failure.

## WebSocket ticket flow

`POST /ws-token` (JWT) → issues `{ "ticket": "<uuid>" }`, stored in `AppState.ws_tickets`.  
`GET /instances/:id/console?ticket=<uuid>` → validated without `Authorization` header.  
**SEC-02 FIXED**: `gc_ws_tickets` task in `main.rs` drains expired tickets every 5 minutes.

## Rules

- No business logic in handlers — delegate to `application/` services.
- Handler return type: `Result<impl IntoResponse, ApiError>`.
- CORS is permissive for development; tighten for production via `ALLOWED_ORIGIN`.
