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
  pipeline.ts       CommunicationPipeline — timeout/retry/queue/relay policy runner
  endpoint-policies.ts Defaults for every Core endpoint key
  pipeline-types.ts Communication surfaces, methods, policies, nodes, results
  queue.ts          MemoryQueueStore + CompositeQueueStore implementations
  ws.ts             CoreWsConnection — typed WebSocket for instance consoles
  transport.ts      Explicit message bus — 4 modes, ack policies, message registry
  convex-relay.ts   Raw Convex HTTP query/mutation calls
  core-relay.ts     Direct calls to Core's /relay/messages endpoint
  connection.ts     verifyCoreConnection — nonce handshake against Core
  monitor.ts        CoreConnectionMonitor — periodically runs the Core handshake
  instance-state.ts CoreInstanceStateManager — reactive state combining all of the above
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
`CoreApiClient.method()` → endpoint key policy → `CommunicationPipeline.callValue()` → builds `CoreCallContext` from adapter → calls raw function in `api.ts` → returns typed result.

`CoreApiClient.request()` resolves method/path metadata into an endpoint key using `resolveCoreEndpointKey()`. `CoreApiClient.withPolicy()` creates a lightweight client wrapper with per-call defaults (timeout, retries, methods, queue, auth mode, relay flags).

**Pipeline rules:**

- Every Core endpoint has a default policy in `endpoint-policies.ts`.
- Direct calls use `core-direct`; async messages use `CommunicationPipeline.publish()`.
- Convex relay is opt-in through `allowConvexRelay`; default policies avoid expensive Convex traffic.
- Queue fallback requires `adapter.queueStore` and a payload. Desktop currently supplies a localStorage-backed queue.
- `throwOnError` defaults true inside the library. App composables catch errors and expose refs so failures do not escape into Tauri uncaught.

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

`CommunicationPipeline.publish()` adds higher-level methods over these modes: `core-relay`, `convex-relay`, `memory-queue`, `persistent-queue`, and `fire-and-forget`. It can wait for received or processed acknowledgements through Core relay or Convex relay based on policy.

`drainQueue()` is **not auto-called** — the consumer must call it when `CoreConnectionMonitor` transitions to `connected`.

---

## Where It Gets Loaded

| Consumer                                                | What it does                                                                                                  |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `apps/app-frontend/src/adapters/desktop.ts`             | Implements `PlatformAdapter` using native `window.fetch`, env URLs, no-auth dev mode, and local queue storage |
| `apps/app-frontend/src/composables/useCoreClient.ts`    | Provides the singleton `CoreApiClient`                                                                        |
| `apps/app-frontend/src/composables/useCoreCall.ts`      | Vue-safe call wrapper; catches errors and exposes reactive refs                                               |
| `apps/app-frontend/src/composables/useCoreMessage.ts`   | Vue-safe message publish wrapper over `CommunicationPipeline.publish()`                                       |
| `apps/app-frontend/src/composables/useCoreInstances.ts` | Instances list + SSE state                                                                                    |

---

## Key Non-Obvious Details

- `CoreApiClient` caches the Core URL promise but **only when non-null** — so if Core is offline, the next call retries discovery instead of caching the miss.
- `types.ts` has no validation — it is purely structural. Drift from Core's Rust structs is silent at runtime.
- `CoreConnectionMonitor` verifies Core with a nonce handshake at `/connection/handshake`. It does not heartbeat and does not use Convex as a Core liveness bus.
- `auth.ts` reads env vars with a dual path: `import.meta.env.VITE_*` for Vite builds, `process.env.*` for Node (tests/scripts). Both paths need the corresponding var set.
