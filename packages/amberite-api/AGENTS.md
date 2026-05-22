# amberite-api

Shared communication library — typed HTTP client, WebSocket, SSE, and explicit message transports for the Amberite desktop app, Core server, and Convex backend. Zero npm dependencies; platform differences are injected via `PlatformAdapter`.

---

## File Structure

```
src/
  adapter.ts        PlatformAdapter contract — fetch, auth, Core URL, queue store
  context.ts        CoreCallContext — resolved baseUrl + token passed to api.ts
  api.ts            Raw typed HTTP functions for every Core endpoint
  client.ts         CoreApiClient class + CoreEventStream (SSE) class
  ws.ts             CoreWsConnection — typed WebSocket for instance consoles
  transport.ts      Explicit message bus — 4 modes, ack policies, message registry
  convex-relay.ts   Raw Convex HTTP query/mutation calls
  core-relay.ts     Direct calls to Core's /relay/messages endpoint
  monitor.ts        CoreConnectionMonitor — pings /health + Convex presence
  instance-state.ts CoreInstanceStateManager — reactive state combining all of the above
  heartbeat.ts      CoreHeartbeat — fires heartbeatCore to Convex every 30s
  drain.ts          drainQueue — flushes direct-queued messages to Core when online
  auth.ts           Microsoft OAuth — startMicrosoftLogin / completeMicrosoftLogin
  errors.ts         Error hierarchy: AmberiteApiError → NetworkError, AuthError,
                    CoreOfflineError, RelayTimeoutError, CoreApiError
  types.ts          TypeScript mirrors of Core's Rust structs — must sync with apps/core/
```

---

## Architecture

Everything flows through `PlatformAdapter`. It is the only interface that differs between desktop (Tauri) and web. The library itself has no platform code.

**Call path for a normal API call:**
`CoreApiClient.method()` → `this.direct()` → builds `CoreCallContext` from adapter → calls raw function in `api.ts` → returns typed result.

`CoreApiClient.request()` accepts a `_relayPayload` second arg that is currently **completely unused** — it's a reserved stub for future relay fallback routing. Don't treat it as active logic.

**Event flow:**
Core pushes SSE events at `GET /events`. `CoreEventStream` reads the `ReadableStream`, splits on `\n\n`, strips `data:` prefixes, and emits typed `CoreInstanceEvent` objects. `CoreInstanceStateManager` subscribes to these and patches its internal `Map<id, CoreInstanceSummary>`.

**WebSocket consoles:**
`issueWsTicket()` must be called first to get a short-lived ticket — the ticket goes in the WS URL query string so auth tokens aren't in WS headers. `CoreWsConnection` dispatches `log`, `stats`, and `state` typed events. Unparseable frames fall back to plain log strings.

---

## Message Transport Modes (`transport.ts`)

| Mode                     | Behavior                                                                                     |
| ------------------------ | -------------------------------------------------------------------------------------------- |
| `direct-fire-and-forget` | Envelope returned; nothing persisted or sent                                                 |
| `direct-queued`          | Stored in `PersistentQueueStore`; `drainQueue()` delivers when Core is online                |
| `core-relay`             | POSTed directly to Core's `/relay/messages`                                                  |
| `convex-relay`           | POSTed to Convex `messaging:publishMessage`; `waitForResult` polls `messaging:messageStatus` |

`drainQueue()` is **not auto-called** — the consumer must call it when `CoreConnectionMonitor` transitions to `online-direct`.

---

## Where It Gets Loaded

| Consumer                                                       | What it does                                                             |
| -------------------------------------------------------------- | ------------------------------------------------------------------------ |
| `apps/app-frontend/src/adapters/desktop.ts`                    | Implements `PlatformAdapter` using Tauri invoke, tauriFetch, OS keychain |
| `apps/app-frontend/src/providers/setup/core-instance-state.ts` | Bootstraps `CoreInstanceStateManager`                                    |
| `packages/ui/src/providers/core-client.ts`                     | Provides `CoreApiClient` via Vue DI                                      |
| `packages/ui/src/providers/core-instance-state.ts`             | Provides `CoreInstanceStateManager` via Vue DI                           |

---

## Key Non-Obvious Details

- `CoreApiClient` caches the Core URL promise but **only when non-null** — so if Core is offline, the next call retries discovery instead of caching the miss.
- `types.ts` has no validation — it is purely structural. Drift from Core's Rust structs is silent at runtime.
- `CoreConnectionMonitor` checks Convex `presence:corePresence` as fallback when direct `/health` fails. It treats presence as stale after 30s.
- `auth.ts` reads env vars with a dual path: `import.meta.env.VITE_*` for Vite builds, `process.env.*` for Node (tests/scripts). Both paths need the corresponding var set.
