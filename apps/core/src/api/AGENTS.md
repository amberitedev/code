# src/api

This directory is intentionally empty. It is not the Core's HTTP API layer.

The previous generic message, envelope, audience, receipt, and relay abstraction was removed because it made peer-to-peer delivery a central primitive without a real product need. It duplicated durable Convex mutations, Core SSE, console WebSockets, and Cloudflare presence while introducing polling, client-controlled routing, and a second authorization model.

Do not recreate that system here. In particular, do not add generic sender/recipient IDs, a message queue, delivery receipts, relay endpoints, polling loops, or a Core-to-client bus.

## Where the real Core API lives

```text
apps/core/src/
  presentation/
    router.rs                        Canonical Axum route registration
    contracts.rs                     HTTP request/response contracts
    extractors.rs                    Authentication and request context extraction
    authz.rs                         Object-level Core authorization helpers
    handlers/                        Thin REST, SSE, and console-WebSocket boundaries
      events.rs                      Authenticated Core event stream
      console.rs                     Console WebSocket ticket and stream
      setup.rs                       First-run pairing
      social.rs, sync.rs             Core-local social/sync HTTP actions
  application/
    pairing_service.rs               Convex-backed Core pairing and credential lifecycle
    social_*.rs, sync_*.rs           Domain/application behaviour behind handlers
  domain/event.rs                    Typed Core-local events consumed by SSE
```

Read `presentation/AGENTS.md` before changing Core-facing HTTP, SSE, or console WebSocket behaviour. Core HTTP is for direct, authenticated Core control. SSE and the console WebSocket are Core-local streams, not replacements for durable social state.

## Cross-service ownership

- Convex is authoritative for identity, friendships, groups, blocks, pairing, and authorization.
- Core owns local instance management, local files, console data, and local event production.
- The Cloudflare realtime Worker owns only ephemeral desktop online state.

When a Core action changes durable social authorization, make the corresponding authenticated Convex mutation the authority. When it changes only local Core state, emit the existing typed Core event and let the authenticated SSE/console stream deliver it. Never invent a second durable representation merely to notify another client.

## Contribution rules

- Add an HTTP route in `presentation/router.rs`, a narrowly typed handler and contract, then put behavior in an application service. Keep handlers thin.
- Authorize every resource ID at the handler/service boundary. A route-level authenticated user is not authorization to every Core, instance, file, or event.
- Prefer a push stream that already exists over page-level polling. Use an explicit post-action reconciliation only where no stream can represent the result safely.
- Coordinate protocol or contract changes with `packages/amberite-api` and the realtime Worker. Desktop clients can lag behind, so preserve compatible wire contracts during rollout.
