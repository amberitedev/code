# packages/convex — Convex backend functions

Convex deployment for Amberite: messaging relay, Core presence, and Convex Auth.

## Structure

```
packages/convex/
  schema.ts       — DB table definitions (authTables + app tables)
  _socialRules.ts — Shared identity/invariant helpers for social group/Core ownership rules
  messaging.ts    — Mode 3 convex-relay functions (publishMessage, pendingMessages, ackMessage, completeMessage, messageStatus)
  presence.ts     — Core registration and heartbeat (registerCore, heartbeatCore, corePresence, friendGroupCores)
  friends.ts      — friend codes, friend requests, friendships, and personal blocks
  friendGroups.ts — group/Core metadata, members, roles, and member permissions
  groupInvites.ts — code and directed friend-group invites
  sync.ts         — synchronized profile registry and snapshot/event scaffolding
  auth.ts         — Convex Auth config + user functions (currentUser, setUsername)
  auth.config.ts  — Auth provider config (MicrosoftEntraID)
  http.ts         — HTTP router for auth callback routes
  convex.json     — Convex project config: "functions": "." (source at package root)
  _generated/     — Codegen output (gitignored — do not edit)
  .convex/        — Local Convex state (gitignored — causes dev loop if not excluded)
  .env.local      — Deployment URL + auth secrets (gitignored — not committed)
```

## Schema tables

| Table                                                 | Purpose                                                             |
| ----------------------------------------------------- | ------------------------------------------------------------------- |
| `users`                                               | Convex Auth user records (extended with `username`)                 |
| `linkedMicrosoftAccounts`                             | Microsoft Entra accounts linked to Convex users                     |
| `cores`                                               | Registered Core instances (coreId, ownerId, convexUrl, etc.)        |
| `messages`                                            | Mode 3 (convex-relay) message envelopes                             |
| `receipts`                                            | Ack/complete records for convex-relay messages                      |
| `friendGroups`                                        | Named groups of users who share Core access                         |
| `friendGroupMembers`                                  | Members of friend groups                                            |
| `pairingCores`                                        | Temporary remote pairing registrations (6-digit code + coreId)      |
| `friendRequests`, `friendships`, `blockedUsers`       | Personal social graph and blocking                                  |
| `friendGroupInvites`                                  | Directed and link-based group invitations                           |
| `syncedProfiles`, `profileSnapshots`, `modSyncEvents` | Mod-sync infrastructure; diff/apply is deferred                     |
| `authTables`                                          | Injected by `@convex-dev/auth` — session, account, verificationCode |

## Dev loop gotcha

`convex.json` sets `"functions": "."` (package root). The Convex watcher uses `.gitignore` to determine watched paths. `.convex/` and `_generated/` MUST be in `.gitignore` — if they're not, writes to those dirs trigger a redeploy which writes again → infinite loop.

Convex HTTP calls use `/api/query` and `/api/mutation` with paths like `friends:ensureSocialProfile`. The bearer token is the Convex Auth session JWT from the desktop keychain; no generated Convex React client is used in `apps/app-frontend`. Desktop login calls `auth:signIn` through the Tauri `amberite` plugin, opens Microsoft OAuth in the system browser, captures the callback with the local loopback listener, and stores both JWT and refresh token in the keychain.

Identity rule: friend groups enforce one membership per user, one Core per owner, and one Core per friend group. Use Convex `users._id`/JWT `sub` for social membership IDs; `amberiteUserId` is a public profile value and is not the auth subject. Reuse `_socialRules.ts` for these checks rather than reimplementing them in individual function files.

## Running

```
pnpm --filter @amberite/convex dev   # or: pnpm convex:dev from root
```

Requires `.env.local` with `CONVEX_DEPLOYMENT` and `CONVEX_DEPLOYMENT_TYPE` set by `convex dev` on first run, plus `JWKS_ENDPOINT` and Microsoft OAuth vars for auth.

## Adding message types

New message definitions live in `packages/amberite-api/src/transport.ts` (`messageDefinitions`). For `convex-relay` mode messages, the Convex `messaging.ts` functions handle storage and retrieval — no changes needed there unless the receipt/status model changes.

## Adding auth providers

Auth providers are configured in `auth.config.ts` (provider list) and `auth.ts` (`convexAuth` call). Add new providers to both files.
