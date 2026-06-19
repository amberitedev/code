# convex

Amberite Convex backend for social identity, friend groups, Core presence, synced profiles, and relay messaging. This directory is deployed from the repo root `convex.json`.

## Critical Workflow

- After changing anything in `convex/`, push it to the dev deployment before calling the work done. This is required even though normal app/core/web dev servers should not be started during agent work:

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

- `resolveActor(ctx, __actAs)` in `_socialRules.ts` is the identity gate. Production and the desktop app must use real Convex Auth/JWT identity. Dev-only `__actAs` is only for explicit backend test utilities and must not be wired into app clients.
- Never persist `__actAs`. Convex document fields beginning with `_` are reserved, so destructure it away before inserts, patches, or metadata copies.
- The one-user/one-Core/one-friend-group invariant is enforced by `requireSingleGroupMembership`, `requireSingleOwnedCore`, `getOrCreateDefaultFriendGroup`, and `upsertCoreForFriendGroup`.
- Friend-group authorization flows through `requireFriendGroupRole`. Owner outranks admin outranks member; use `roleRank` for operations that compare two members.
- Public user responses should go through `publicUser` so callers do not receive raw auth/user documents.
- Queries with `returns` validators must return explicit public objects, not raw Convex documents with `_id`/`_creationTime`, unless the validator allows those fields.

## Auth Architecture

Amberite identity is the same identity as the user's Minecraft login. There is no separate Microsoft OAuth app registration or env-configured auth exchange URL in the desktop app.

- The desktop app authenticates to Minecraft using the upstream Modrinth hardcoded Xbox client ID (`packages/app-lib/src/state/minecraft_auth.rs`).
- When the app needs an Amberite session, it reads the active Minecraft credentials from the local Rust state and sends the Minecraft access token to Convex.
- Convex exposes a `ConvexCredentials` provider (`minecraft-token`) in `convex/auth.ts`.
- The provider verifies the token with `https://api.minecraftservices.com/minecraft/profile`, then finds or creates an Amberite user linked by Minecraft UUID.
- Convex Auth issues the session JWT, which the desktop app stores and sends as `Authorization: Bearer <token>`.

This keeps the desktop app's auth surface minimal: no client secrets, no browser OAuth redirects, and no extra Microsoft login screen.

## App Connection Path

- Desktop app adapter: `apps/app-frontend/src/adapters/desktop.ts`
- Shared Convex client: `packages/amberite-api/src/convex-api.ts`
- Raw HTTP transport: `packages/amberite-api/src/convex-relay.ts`
- The desktop app stores a real Amberite session JWT and sends it as `Authorization: Bearer <token>`.
- Live endpoint shapes:

```json
{ "path": "friends:friendsList", "args": {}, "format": "json" }
{ "path": "auth:signIn", "args": { "provider": "minecraft-token", "params": { "minecraftAccessToken": "..." } }, "format": "json" }
```

sent as `POST /api/query`, `/api/mutation`, or `/api/action` on `CONVEX_URL`.

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

- `dev.ts` functions are intentionally inert without `AMBERITE_DEV_MODE`.
- Production/preview deployments and app clients must not rely on `__actAs`; real auth should be present.
- If `schema.ts` changes, push with `pnpm exec convex dev --once --tail-logs disable` and then run at least one targeted function through `pnpm exec convex run` or `/api/query`/`/api/mutation`.
