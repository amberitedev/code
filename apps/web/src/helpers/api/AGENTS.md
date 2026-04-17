# helpers/api/ — API Layer

HTTP client (`client.ts`) and WebSocket manager (`websocket.ts`) for communicating with Amberite Core.

## Files

| File | Purpose |
|------|---------|
| `types.ts` | TypeScript types for all Core API shapes |
| `client.ts` | ofetch wrapper with API key interceptor |
| `websocket.ts` | WebSocket manager with auto-reconnect |

## Connection

- **Core URL:** `http://localhost:16662` (or from settings `core_url`)
- **Auth:** API key attached via `Authorization: Bearer <key>` header
- **WebSocket:** `ws://<host>:<port>/instances/:id/console?key=<api_key>`

## Events

The client emits events via `system/events.ts`:
- `auth-failed` — 401 response, key may be invalid
- `connection-lost` — Network error
- `connection-restored` — Reconnected successfully
