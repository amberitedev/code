# convex

Amberite Convex backend for social identity, friend groups, Core presence, synced profiles, and relay messaging. This directory is deployed from the repo root `convex.json`.

## Critical Workflow

- After changing anything in `convex/`, push it to the dev deployment before calling the work done:

```powershell
pnpm exec convex dev --once --tail-logs disable
```

- Confirm the target deployment before relying on it:

```powershell
pnpm exec convex env list
pnpm exec convex run dev:listDevUsers
```

- The current dev app expects `.env.local` at repo root to provide `CONVEX_DEPLOYMENT` and `CONVEX_URL`. Vite maps `CONVEX_URL` to `VITE_CONVEX_URL` in `apps/app-frontend/vite.config.ts`, and the desktop adapter sends requests to `${VITE_CONVEX_URL}/api/query` and `/api/mutation`.
- Do not start long-running `convex dev` watchers unless explicitly asked. Use `--once` for agent work.
- Do not commit or document real secrets from `.env.local`.

## Navigation Map

```text
convex/
  schema.ts          Tables, validators, and indexes for auth, users, groups, cores, sync, messages
  _socialRules.ts    Shared auth/authorization helpers, group/core invariants, public user shaping
  auth.ts            Auth/current user functions and username setup
  dev.ts             Dev-only seed/reset/state helpers guarded by AMBERITE_DEV_MODE
  friends.ts         Social profiles, search, heartbeat, friend requests, blocks
  friendGroups.ts    Core-owned friend groups, roles, bans, ownership transfer, group Core listing
  groupInvites.ts    Direct and code-based friend-group invites
  presence.ts        Core registration, Core presence, pairing-code claim flow
  sync.ts            Synced server profiles, visibility, auto-whitelist resolution, snapshots
  messaging.ts       Authenticated Core/user relay messages and receipts
  http.ts            HTTP route registration, if any external HTTP endpoints are added
  _generated/        Convex-generated types; update through Convex codegen/dev, do not hand-edit
```

## Core Logic

- `resolveActor(ctx, __actAs)` in `_socialRules.ts` is the identity gate. In production it requires real Convex Auth. In dev only, `AMBERITE_DEV_MODE=true` allows the desktop app to pass `__actAs` so it can act as seeded users before Microsoft auth is wired.
- Never persist `__actAs`. Convex document fields beginning with `_` are reserved, so destructure it away before inserts, patches, or metadata copies.
- The one-user/one-Core/one-friend-group invariant is enforced by `requireSingleGroupMembership`, `requireSingleOwnedCore`, `getOrCreateDefaultFriendGroup`, and `upsertCoreForFriendGroup`.
- Friend-group authorization flows through `requireFriendGroupRole`. Owner outranks admin outranks member; use `roleRank` for operations that compare two members.
- Public user responses should go through `publicUser` so callers do not receive raw auth/user documents.
- Queries with `returns` validators must return explicit public objects, not raw Convex documents with `_id`/`_creationTime`, unless the validator allows those fields.

## App Connection Path

- Desktop app adapter: `apps/app-frontend/src/adapters/desktop.ts`
- Shared Convex client: `packages/amberite-api/src/convex-api.ts`
- Raw HTTP transport: `packages/amberite-api/src/convex-relay.ts`
- Dev acting-user storage key: `amberite:dev:actingUserId`
- Live endpoint shape:

```json
{ "path": "friends:friendsList", "args": { "__actAs": "..." }, "format": "json" }
```

sent as `POST /api/query` or `POST /api/mutation` on `CONVEX_URL`.

## Useful Checks

Run from repo root:

```powershell
pnpm exec convex dev --once --tail-logs disable
pnpm exec convex run dev:listDevUsers
pnpm exec convex run dev:devState
pnpm --filter @amberite/amberite-api test
```

Live app-style smoke test pattern:

```powershell
node --env-file=.env.local -e "fetch(process.env.CONVEX_URL + '/api/query', { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify({ path: 'dev:listDevUsers', args: {}, format: 'json' }) }).then(r => r.json()).then(console.log)"
```

`dev:resetSocial` wipes social/Core/sync test state while keeping users by default when called with `{"includeUsers":false}`.

## Deployment Notes

- Dev deployment env must include `AMBERITE_DEV_MODE=true` for seeded-user desktop testing.
- `dev.ts` functions are intentionally inert without `AMBERITE_DEV_MODE`.
- Production/preview deployments must not rely on `__actAs`; real auth should be present.
- If `schema.ts` changes, push with `pnpm exec convex dev --once --tail-logs disable` and then run at least one targeted function through `pnpm exec convex run` or `/api/query`/`/api/mutation`.
