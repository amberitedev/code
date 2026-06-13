# src/api

Typed communication layer for Core messages, envelopes, relay storage, and distribution.

This layer defines the vocabulary and routing semantics for Core-to-app and Core-to-Core style communication. It does not own Axum routing; HTTP handlers live in `src/presentation`.

## Mental Model

A message has:

- typed IDs for who or what it refers to
- a stable `MessageKind` wire name
- a payload struct
- route, acknowledgement, and durability policy
- an envelope used for transport and storage

The API layer separates “what happened or should happen” from “how HTTP exposes it.”

Relay storage is durable message persistence. Distribution decides who receives message copies. Handlers call into this layer when they need to enqueue, fetch, acknowledge, or inspect messages.

You usually do not need to read every file in this folder. Pick based on which part of the system is changing.

## File Relationships

- `ids.rs` defines stable typed IDs shared by messages and relay code.
- `kind.rs` defines message wire names.
- `policy.rs` defines route, acknowledgement, and durability concepts.
- `message.rs` defines the trait tying a payload to its kind and policy.
- `messages.rs` defines concrete message payloads.
- `envelope.rs` wraps typed messages for transport/storage.
- `store.rs` defines relay persistence and membership traits.
- `relay_store.rs` implements relay persistence with SQLite.
- `distributor.rs` expands an audience into stored message copies.
- `mod.rs` re-exports the public API surface.

## Common Changes

| Change | Read |
| ------ | ---- |
| Add a new message type | `kind.rs`, `messages.rs`, `message.rs`, `mod.rs` |
| Change stored relay behavior | `relay_store.rs`, `store.rs` |
| Change fan-out logic | `distributor.rs`, `audience.rs` |
| Change envelope format | `envelope.rs` |
| Change HTTP relay endpoint | `src/presentation/handlers/relay.rs` plus this file |
