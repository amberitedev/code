# amberite-api

Amberite's shared frontend service layer and the primery coms to its backends. The desktop app and dashboard share backend clients, contracts, validation, data normalization, and reusable backend-facing workflows here. This layer should turn backend protocols into information and operations that are easy for either frontend to use. Platform adapters may supply the actual transport. `packages/api-client` remains separate because it talks to Modrinth, not Amberite.

## Important parts

| Area | Purpose |
| --- | --- |
| `api.ts`, `client.ts`, `types.ts` | Direct Core HTTP operations, SSE client, and wire DTOs |
| `ws.ts`, `connection.ts` | Core console WebSocket and connectivity handshake |
| `realtime.ts` | Realtime socket lifecycle and server-frame validation |
| `convex-types.ts`, `social-session.ts` | Public Convex DTOs and pure durable/live composition with authorization pruning |
| `auth-client.ts`, `session.ts`, `profile.ts` | Shared authentication, session storage contracts, and profile normalization |
| `logic/` | Product workflows and transformations shared by the desktop app and dashboard |
| `context.ts`, `adapter.ts`, `errors.ts` | Injected platform capabilities and safe client errors |
| `index.ts` | Deliberate public exports |

`convex-api.ts`, the relay/transport/queue/pipeline files, `monitor.ts`, and `instance-state.ts` are older compatibility paths. Prefer narrow public clients and shared workflows for new work instead of exposing transport machinery to frontends.

## Rules

- Put backend-facing logic here when it can be shared or keeps backend knowledge out of UI code: calls, validation, normalization, combining responses, and reusable workflows. Frontends should normally use these APIs instead of talking to Core, Convex, or Realtime directly.
- Apply that boundary sensibly. UI-only state, presentation formatting, route behavior, and platform wiring stay in the app; do not create useless wrappers for trivial one-off UI behavior.
- Keep the package platform-neutral. Inject `fetch`, WebSocket creation, token lookup, and storage instead of importing Vue, Tauri, browser globals, or Convex client implementations.
- Validate external data and preserve compatibility. When a backend contract changes, update its producer, types, parser, consumer, and focused tests together; the owning guides are `convex/AGENTS.md`, `apps/realtime/AGENTS.md`, and `apps/core/AGENTS.md`.
