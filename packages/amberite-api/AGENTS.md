# amberite-api

Shared, platform-neutral communication boundary for Amberite desktop and web clients. This package owns Amberite-specific wire contracts, direct Core transport primitives, realtime protocol validation, and pure durable/live state composition. It does not own Vue state, browser/Tauri globals, Convex subscription lifetime, durable storage, or a generic peer-to-peer messaging system.

## Architecture

```text
ConvexClient subscription ─┐
                           ├─ durable SocialSessionState input
Worker WebSocket frames ───┘
                           │
                  composeSocialSessionState()
                           │
                        UI state

Direct Core HTTP / SSE / console WebSocket
                           │
                  focused Core control operations
```

Convex is authoritative for durable identity, profiles, relationships, linked Modrinth accounts, minimal Core list projection, pairing, and sync metadata. Core is authoritative for real Core roles, permissions, bans, invites, instance access, and audit logs. The Cloudflare Worker is authoritative only for ephemeral desktop online state. This package merges those independently delivered inputs without granting new authority: live users absent from the latest durable authorization set are removed before UI code receives state.

The desktop/web integration owns a real `ConvexClient` subscription and passes its result into the composer. A social mutation changes durable Convex state; the subscription converges it. Do not follow every mutation with a full refresh. The Worker socket owns no durable state and reconnects with a bounded backoff after genuine disconnects.

Direct Core calls remain appropriate for Core-local actions and snapshots. Core SSE and console WebSockets deliver watched local changes. They are not a generic social transport.

## Non-negotiable rules

- Do not add polling, heartbeat loops, timer-based full refreshes, or "event then fetch again" paths. Convex function calls must correspond to user actions or real durable changes, not elapsed open-app time.
- Do not add a peer-to-peer role model, arbitrary `senderId`/`recipientId`, relay envelope, delivery receipt, or offline message queue. There is no product requirement for a generic transport bus.
- Do not use `ConvexApiClient` or raw Convex HTTP calls as the normal desktop state transport. Use one-shot calls only for auth bootstrap, debounced search, and callers that cannot subscribe.
- Keep this package dependency-free and platform-neutral. Inject `fetch`, WebSocket creation, token lookup, and storage through narrow interfaces; never import Vue, Tauri, browser globals, or a Convex client implementation here.
- Validate every external frame and DTO at the boundary. Do not let untyped `unknown` or raw backend documents reach UI consumers.
- Model contracts explicitly and preserve backward compatibility. A desktop client may be older than the Core or Worker it connects to.
- Do not merge this package into `packages/api-client`. That package is a Modrinth API client with Modrinth module and feature semantics; sharing a TypeScript runtime does not create a shared domain boundary.

## Supported boundary and migration status

```text
src/
  social-session.ts      Pure durable + live social-state composer and authorization pruning
  realtime.ts            Desktop Worker ticket/socket lifecycle, frame validation, bounded reconnect
  convex-types.ts        Public durable Convex wire DTOs
  api.ts                 Direct typed Core HTTP operations
  client.ts              Core API client and Core SSE event stream
  ws.ts                  Core console WebSocket contract
  connection.ts          One-shot Core connectivity handshake
  context.ts, adapter.ts Narrow injected platform/Core-call context
  errors.ts              Shared, safe client error types
  types.ts               Core HTTP/SSE/console DTOs; keep aligned with Core contracts
  index.ts               Deliberate public exports only

  convex-api.ts          Legacy raw Convex HTTP facade; do not extend
  convex-relay.ts        Legacy Convex relay transport; remove after migration
  core-relay.ts          Legacy Core relay transport; remove after migration
  transport.ts           Legacy generic message model; remove after migration
  pipeline*.ts,
  endpoint-policies.ts   Legacy policy/retry/queue abstraction; remove after migration
  queue.ts, drain.ts     Legacy offline generic-message queue; remove after migration
  monitor.ts,
  instance-state.ts      Legacy state ownership; do not add new polling/state orchestration
  auth.ts                Legacy Microsoft OAuth path; do not extend
  logic/                 App workflow helpers; move non-transport workflows to their owning app
```

## How to change this area

For durable social or Worker presence changes, read `social-session.ts`, `realtime.ts`, the matching `convex/social.ts`, `convex/coreList.ts`, `convex/realtimeBridge.ts`, and `apps/realtime/src/index.ts` together. Update producer, consumer, validator, and tests as one protocol change.

Durable shell state is profile, friends, Core list/member-link projection, and live user presence only. UI that needs actual Core permissions, roles, bans, invites, or audit logs must fetch them directly from Core.

For Core control changes, read the endpoint in `apps/core/src/presentation/router.rs`, its handler and contract, then update the narrowly matching types/client call here. The Core's SSE and console WebSocket schemas must change with their Rust producers and consumer validation.

Put app-specific UI formatting and Modrinth install workflows in the app, not here. The correct abstraction is a small, pure contract/composition library, not a universal client framework.

## Consumers and checks

The desktop adapter and composition live under `apps/app-frontend/src/adapters/desktop.ts` and `apps/app-frontend/src/composables/useSocial.ts`. Web consumers must use the same contracts but supply their own platform bindings.

Run the focused package tests after code changes:

```powershell
pnpm --filter @amberite/amberite-api test
```

Add integration coverage for social-state composition, authorization pruning, malformed Worker frames, reconnect/disposal, Core contracts, and subscription convergence. Do not make an integration test start a Core development server as hidden test setup.
