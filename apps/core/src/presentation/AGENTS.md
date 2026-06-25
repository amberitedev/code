# src/presentation

The Core's real API boundary: Axum REST endpoints, authenticated Server-Sent Events, and the console WebSocket. This layer translates protocol input into application calls; it does not own durable social state, generic messaging, or client-side state management.

## Communication model

Core's HTTP API is direct control of the paired Core: instances, files, configuration, roles, sync operations, and related local state. User actions call a focused endpoint. Existing streams push local state changes:

- `handlers/events.rs` exposes authenticated Core instance events through SSE.
- `handlers/console.rs` issues a short-lived ticket and hosts the console WebSocket.

There is no generic Core relay endpoint or `src/api` message layer. Do not reintroduce envelopes, arbitrary recipient IDs, delivery receipts, Core-to-Core delivery, or polling to compensate for a missing state design. Durable social state belongs in Convex; live presence belongs in the Worker; Core-local facts are streamed by SSE/console when a consumer is watching them.

## Navigation map

```text
src/presentation/
  router.rs              Canonical route and middleware registration
  contracts.rs           Request/response DTOs shared by handlers
  extractors.rs          Bearer/session extraction and typed caller context
  authz.rs               Core, role, and instance authorization checks
  error.rs               Safe conversion of application errors to HTTP responses
  handlers/
    diagnostics.rs       Health, version, handshake
    setup.rs             First-run setup and pairing
    events.rs            Authenticated Core SSE stream
    console.rs           Console WebSocket ticket and stream
    social.rs, access.rs, roles.rs, invites.rs
                         Core-local access and collaboration actions
    sync.rs               Core-local sync profile/snapshot actions
    instances.rs, instance_control.rs
                         Instance reads and lifecycle mutations
    fs.rs, logs.rs, mods.rs, modpack.rs, players.rs, properties.rs,
    tasks.rs, backups.rs, rcon.rs, stats.rs, query.rs, installations.rs,
    macros.rs             Focused Core-control endpoints
```

## Contribution rules

- Start at `router.rs` to see the public route and then read the matching handler, contract, authorization helper, and application service. Do not create a catch-all communication endpoint.
- Keep handlers thin: parse/validate input, authenticate and authorize each referenced object, call an application service, and map known errors through `error.rs`.
- A stream subscriber must be authorized for the specific Core and instance data it receives. Recheck or terminate long-lived streams when access is revoked.
- Use typed event payloads from `src/domain/event.rs`; do not send arbitrary JSON frames or make a page poll alongside an existing event stream.
- Required configuration must fail fast. Never substitute a production origin, audience, or service endpoint when configuration is invalid.
- When an endpoint's durable social permissions or wire contract changes, coordinate with Convex and `packages/amberite-api`; preserve a compatible contract while desktop clients roll forward.
