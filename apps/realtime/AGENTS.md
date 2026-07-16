# realtime

Cloudflare Worker and Durable Object for ephemeral desktop presence. It owns short-lived session tickets, open WebSockets, online/offline transitions, and authorized fan-out. Durable authorization is resolved through the private Convex bridge.

## Runtime shape

`PresenceHub` is one deliberately unsharded Durable Object named `global-v1` per environment. Tag-based WebSocket fan-out reaches recipient devices without scanning every socket. Stored ticket, rate-limit, and session-expiry data is opaque, TTL-bound, and deleted on consume, close, or expiry.

1. `POST /v1/desktop-sessions` validates the Convex JWT signature, expiry, issuer, audience, and subject; asks the bridge for the viewer's visibility scope; then issues a single-use ticket.
2. `GET /v1/connect?ticket=…` atomically consumes the ticket and upgrades to a hibernating WebSocket.
3. `POST /v1/invalidate` is the authenticated service path that terminates affected sessions after authorization changes.

The server frame set is closed: `presence.snapshot`, `presence.user`, and `authorization.invalidated`. Clients send no application messages; reject client frames after a bounded size check. Presence changes only on first/last desktop socket transitions.

## Rules

- Keep durable social, authorization, Core, and relationship data out of this service. Do not add D1, KV, R2, a general queue, periodic writes, polling, heartbeats, socket scans, broadcast-all fan-out, or client-controlled audiences. Normal socket transitions must not call Convex.
- Socket attachments contain only connection identity, session identity, and expiry. Do not attach JWTs, credentials, scopes, or user data beyond what a hibernating callback needs.
- Tickets are random, short-lived, single-use, and never logged. Validate configured desktop origins, rate-limit ticket creation, bound inputs before parsing, and never log bearer tokens or bridge material.
- Keep ticket consumption and lifecycle changes atomic. Do not hold a concurrency block across network I/O; await or explicitly schedule every asynchronous operation and keep cleanup bounded.
- Worker deploys disconnect sockets. Desktop clients must use capped exponential-backoff reconnects; a disconnected presence channel must not block durable social UI.
- Keep `Env` generated from `wrangler.jsonc`. Regenerate `src/worker-configuration.d.ts` after binding changes; secrets remain Worker secrets, never config literals.

## Important parts

| Area | Purpose |
| --- | --- |
| `src/index.ts` | Routes, tickets, JWT/bridge checks, `PresenceHub`, and the server protocol |
| `wrangler.jsonc` | Environments, Durable Object binding/migration, and non-secret variables |
| `src/worker-configuration.d.ts` | Generated Worker bindings and runtime types; never hand-edit |
| `packages/amberite-api/src/realtime.ts` | Client parser and reconnect lifecycle |
| `convex/realtimeBridge.ts`, `convex/bridge.ts` | Private authorization bridge |

The server protocol is defined here. When it changes, update its client contract and Convex bridge as applicable, then test both producer and consumer behavior. Use separate development and production Worker environments.
