# convex

Amberite's durable backend. Convex is auth-required by default and is authoritative for Amberite identity, sessions, profiles, friends, blocks, linked Modrinth accounts, encrypted Modrinth tokens, Core pairing credentials, minimal Core list projection, and cross-device sync metadata. Core remains authoritative for Core roles, permissions, bans, invites, instance access, and audit logs. Convex is not a presence server, a generic message bus, or a timer-driven client cache.

## Communication architecture

```text
Desktop / web dashboard
  ├─ one ConvexClient subscription for durable social state
  ├─ user actions as authenticated Convex mutations
  └─ direct WebSocket to the realtime Worker for ephemeral presence

Core
  └─ direct HTTP/SSE/WebSocket API for local Core control

Cloudflare Worker
  └─ authenticated bridge calls to Convex only for startup scope resolution,
     invalidation, and recipient lookup; normal online/offline transitions
     do not read or write Convex
```

The normal desktop session uses `social.sessionState` as one authorization-preserving durable subscription. Convex reruns it only when its durable dependencies change. Social mutations change durable state and let that subscription converge; they must not trigger a client-wide refresh chain.

Use narrowly scoped subscriptions only while the relevant screen is open, such as `sync.serverProfilesState` and `sync.profileState`. Keep expensive, action-oriented work such as whitelist resolution on demand. One-shot HTTP requests remain acceptable for auth bootstrap, debounced search, and non-subscription callers, but are not the default state transport.

## Non-negotiable rules

- Do not add heartbeats, presence tables, client polling, periodic refreshes, or "event then fetch again" flows. Time spent with the app open must not create Convex function calls.
- Do not use `messaging.ts`, relay tables, receipts, or a generic recipient/sender envelope for new functionality. Durable user actions are mutations; transient desktop online state belongs in the realtime Worker.
- A client never supplies authority. Every query, mutation, action, bridge request, and returned field must derive authorization from the authenticated identity and the current database state.
- Return explicit public DTOs. Never return raw user, group, Core, or membership documents when they contain credentials, hashes, private metadata, or internal-only fields.
- Convex is the only owner of durable social authorization. Cloudflare may cache only connection-lifetime or TTL-bound session data and must ask the bridge for current fan-out recipients.
- Keep queries bounded and indexed. Do not replace a composed subscription with an N+1 client fan-out.
- Treat schema changes as migrations. Widen, backfill, switch callers, then remove old fields/tables; do not combine a destructive schema change with an incompatible client release.

## Realtime bridge

`realtimeBridge.ts` is a service-only HTTP endpoint. The Worker signs bounded requests with `REALTIME_BRIDGE_HMAC_SECRET`; its request ID and timestamp bound replay risk. It exposes only two operations:

- `desktopScope`: resolve a JWT-authenticated user's visible friends and group members.
- `recipients`: resolve the current authorized audience for an actual desktop lifecycle transition.

The bridge is never a public desktop or Core API. Do not add Core presence, general data reads, relay delivery, or regular polling to it. Core credentials are random secrets returned once during pairing; store only their hash here, rotate them on ownership transfer/unpairing, and never return the hash in a public DTO. Core projection sync uses the dedicated HTTP endpoint in `coreProjection.ts`, not generic Convex mutations.

## Navigation map

```text
convex/
  schema.ts            Durable table definitions, validators, indexes, and bridge replay records
  auth.ts              Minecraft-backed Convex Auth and first-account social initialization
  _socialRules.ts      Identity, authorization, invariants, and public DTO helpers
  profiles.ts          Auth-required Amberite profile reads/search/update
  social.ts            sessionState subscription and live visibility-scope derivation
  friends.ts           Friend/profile/search/block mutations and queries
  coreList.ts          Auth-required minimal Core list and projected member-link reads
  coreProjection.ts    Credential-authenticated Core projection HTTP endpoint
  modrinth.ts          Linked Modrinth metadata, token encryption, and reconnect status
  friendGroups.ts      Deprecated migration table API; Core is the new authority
  groupInvites.ts      Deprecated migration invite API; Core is the new authority
  presence.ts          Pairing and Core credential lifecycle; not live presence
  sync.ts              Scoped durable sync-profile/snapshot/event state
  bridge.ts            Internal desktop-scope queries used only by the realtime bridge
  realtimeBridge.ts    HMAC-authenticated Worker HTTP boundary and replay protection
  crons.ts             Bounded cleanup of bridge replay records
  http.ts              Registration for external Convex HTTP endpoints
  messaging.ts         Legacy relay code; do not extend and remove after migration
  dev.ts               Development-only helpers guarded by AMBERITE_DEV_MODE
  _generated/          Generated Convex bindings; never hand-edit
```

## How to change this area

1. Start with `schema.ts`, `_socialRules.ts`, and the affected domain file. Read `social.ts` as well when a change affects a durable entity that appears in the shell.
2. Add or reuse an indexed, authorization-preserving query; extend `sessionState` only for state every signed-in shell needs. Otherwise add a screen-scoped query with a strict bound.
3. Make mutations idempotent where retries or double-clicks are possible. Enforce ownership, membership, block, ban, and role invariants on the server.
4. If a durable authorization change affects desktop live visibility, ensure the next Worker transition resolves recipients through `bridge.ts`; the desktop composer must discard now-unauthorized live entries immediately.
5. Update the public TypeScript contract in `packages/amberite-api` and test both authorization and subscription convergence.

## Deployment and checks

Do not start a long-running watcher. After every source change under `convex/`, deploy the development target before completing the task:

```powershell
pnpm exec convex dev --once --tail-logs disable
```

Confirm the target deployment first with `pnpm exec convex env list`. Never document or commit secrets from `.env.local`. Use the migration component and an expand/migrate/narrow rollout for breaking schema work.
