# presentation/

Axum HTTP layer — routes, extractors, error mapping.

## Files

| File | Purpose |
|------|---------|
| `router.rs` | `create_router(state)` — wires all routes |
| `error.rs` | `ApiError` — unified Axum error type with `From` impls |
| `extractors.rs` | `AuthUser` — validates Supabase JWT and injects `Claims` |
| `handlers/` | One file per feature area (see handlers/AGENTS.md) |

## Route map (summary)

| Method | Path | Auth |
|--------|------|------|
| GET | `/health`, `/version`, `/java` | no |
| GET | `/setup/status` | no |
| POST | `/setup` | no |
| POST | `/ws-token` | yes |
| GET/POST | `/instances` | yes |
| GET/DELETE | `/instances/:id` | yes |
| POST | `/instances/:id/start\|stop\|kill\|restart` | yes |
| POST | `/instances/:id/command` | yes |
| GET | `/instances/:id/console` (WS) | ticket |
| GET | `/instances/:id/progress` (SSE) | yes |
| GET/POST/DELETE | `/instances/:id/modpack` | yes |
| GET | `/instances/:id/modpack/export` | yes |
| GET/POST | `/instances/:id/macros` | yes |
| DELETE | `/instances/:id/macros/:pid` | yes |
| GET/POST | `/instances/:id/mods` | yes |
| POST | `/instances/:id/mods/upload` | yes |
| POST | `/instances/:id/mods/update-all` | yes |
| DELETE/PATCH | `/instances/:id/mods/:filename` | yes |
| PUT | `/instances/:id/mods/:filename/update` | yes |
| GET | `/instances/:id/logs` | yes |
| GET | `/instances/:id/logs/:filename` | yes |
| GET | `/instances/:id/crash-reports` | yes |
| GET | `/instances/:id/crash-reports/:filename` | yes |
| GET/PATCH | `/instances/:id/properties` | yes |
| GET | `/instances/:id/stats` | yes |

## `ApiError`

```rust
pub enum ApiError {
    NotFound(String),
    BadRequest(String),
    Unauthorized(String),
    Internal(String),
    // ... From<InstanceError>, From<ModError>, From<LogError>, etc.
}
```
Implements `IntoResponse` → JSON `{"error": "..."}` with appropriate HTTP status.

## `AuthUser` extractor

Reads `Authorization: Bearer <token>`, validates via `JwksCache`, returns `AuthUser(Claims)`.  
Returns `401` if not paired, token invalid, or missing.

## WebSocket ticket flow

`POST /ws-token` → `{"ticket": "<uuid>"}` stored in `AppState.ws_tickets`.  
`GET /instances/:id/console?ticket=<uuid>` → upgrade without `Authorization` header.
