# src/api — typed message layer

Owns the vocabulary for everything Core sends or distributes. `presentation` serves
these over HTTP; this layer holds no Axum/HTTP code.

## File structure

```
api/
  mod.rs           — re-exports the public surface
  connection.rs    — typed request/response for POST /connection/handshake
  ids.rs           — newtype ids: UserId, CoreId, GroupId, InstanceId, MessageId
  policy.rs        — Route, Ack, Durability enums (+ wire strings)
  status.rs        — DeliveryStatus lifecycle
  kind.rs          — MessageKind closed set (+ dotted wire names)
  audience.rs      — Endpoint, Audience
  message.rs       — Message trait (compile-time route/ack/durability per kind)
  messages.rs      — concrete messages + their Message impls
  envelope.rs      — Envelope + seal::<M>() / decode::<M>()
  error.rs         — ApiCommError
  store.rs         — RelayStore, MembershipResolver async traits
  relay_store.rs   — SqliteRelayStore + StoredMessage (only place with relay SQL)
  distributor.rs   — Distributor: post-and-distribute fan-out engine
```

## How it is used

- `presentation/handlers/relay.rs` builds a `SqliteRelayStore` from `state.pool` and
  calls `insert` / `pending` / `get` / `mark`. Status strings come from
  `DeliveryStatus::wire()`, never literals.
- Typed posts: `Envelope::seal::<M>()` stamps an envelope from a `Message`'s policy
  constants; `Distributor::post` expands an `Audience` and enqueues one durable copy
  per recipient. Wired in as the post-and-distribute flows land.

## Routes

`Route` values and their meaning: `CoreDirect` (request/response), `CorePost`
(post a fact, Core distributes), `CoreRelay` (one durable message to one recipient),
`ConvexRelay` (friend invites + core pairing only), `Local` (stays on machine),
`PeerToPeer` (reserved, no executor — rejected at runtime).

## Adding a message

1. Add a `MessageKind` variant with its dotted wire name.
2. Add a payload struct in `messages.rs` and `impl Message` with its policy constants.
3. Re-export from `mod.rs`.

## Facts

- `StoredMessage` matches the `core_relay_messages` columns and the wire shape the app
  already consumes (`type`, `sender_id`, `recipient_id`, `ack`, `status`, …).
- `result`/`error` are only populated by `get` (the status endpoint), absent from the
  `pending` list shape.
- Relay SQL lives only in `relay_store.rs`.
