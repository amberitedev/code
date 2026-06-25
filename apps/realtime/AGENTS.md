# realtime

Cloudflare Worker and Durable Object for Amberite's ephemeral live state. It is deliberately separate from Convex so online/offline transitions and reconnects do not become durable database writes or subscription churn.

## Communication architecture

Convex owns durable identity, social authorization, pairing, credentials, Core projection, and sync state. This Worker owns only user online presence:

- authenticated, short-lived session tickets;
- open desktop WebSockets;
- desktop online/offline state derived from socket lifecycle; and
- targeted fan-out to currently authorized sockets.

`PresenceHub` is one deliberately unsharded Durable Object named `global-v1` per environment. This is an explicit initial-scale decision: tag-based WebSocket fan-out reaches recipient devices without scanning all sockets. Do not add Core tracking, Core sessions, Core health, relationship data, a durable user store, D1, KV, R2, or a general queue here. Stored ticket, rate-limit, and session-expiry data is opaque and TTL-bound; delete it on consume, close, or expiry.

```text
Desktop ── POST desktop session + WSS ── PresenceHub
                       │
     signed bridge only │
                       ▼
                    Convex
```

## Session and event protocol

1. `POST /v1/desktop-sessions` validates a Convex JWT (signature, expiry, issuer, audience, and subject), asks the Convex bridge for the viewer's visibility scope, then issues a single-use, short-lived ticket.
2. `GET /v1/connect?ticket=…` atomically consumes the ticket and upgrades to a hibernating WebSocket.
3. `POST /v1/invalidate` is the authenticated service path that terminates affected desktop sessions after authorization invalidation.

Server frames are deliberately small and closed:

- `presence.snapshot`
- `presence.user`
- `authorization.invalidated`

Desktop clients send no heartbeat or regular refresh messages. Reject all client messages beyond bounded frame-size checks. Presence changes only on first/last desktop socket transitions.

## Non-negotiable rules

- Never use Cloudflare as durable authorization or durable social storage. Convex resolves the audience at connection and service invalidation/recipient lookup only; normal online/offline transitions must not call Convex.
- Do not add socket scans, broadcast-all fan-out, periodic presence writes, polling, or client-controlled audiences.
- Socket attachments contain only connection identity, session identity, and expiry. Do not attach JWTs, credentials, scopes, or user data beyond what a hibernating callback needs.
- Tickets are random, short-lived, single-use, and never logged. Validate request origin against the configured desktop origins and rate-limit ticket creation.
- Bound every input before buffering or parsing it. Do not log bearer tokens, tickets, or signed bridge material.
- Worker deploys disconnect sockets. Desktop clients must use capped exponential-backoff reconnects; a disconnected presence channel must not block durable social UI.
- Keep `Env` generated from `wrangler.jsonc`. Regenerate `src/worker-configuration.d.ts` after binding changes; secrets remain Worker secrets, never config literals.

## Navigation map

```text
apps/realtime/
  AGENTS.md                         This architecture and contribution guide
  wrangler.jsonc                    Environments, Durable Object binding/migration, non-secret vars
  package.json                      Worker scripts and Worker-runtime test dependencies
  src/index.ts                      HTTP routing, ticket flow, JWT/bridge checks, PresenceHub, protocol
  src/worker-configuration.d.ts     Generated Wrangler binding/runtime types; do not hand-edit
```

The matching contracts are `packages/amberite-api/src/realtime.ts` (desktop socket validation and reconnect ownership) and `convex/realtimeBridge.ts` plus `convex/bridge.ts` (the only authorization bridge).

## How to change this area

Read `src/index.ts`, `wrangler.jsonc`, the matching Convex bridge files, and the relevant client contract before changing a protocol field or scope. Change all producers, consumers, frame validation, and tests together.

Keep Durable Object methods atomic around ticket consumption and lifecycle state. Do not hold a concurrency block across network I/O. Every asynchronous operation must be awaited or explicitly scheduled. Use bounded cleanup; an alarm must not require an unbounded storage scan.

Test ticket single use, JWT rejection, friend/group-only fan-out, blocks/removals, authorization invalidation, hibernation recovery, close cleanup, and reconnect after a Worker deployment. Use a separate development namespace for remote testing and a separate production environment for deployment.
