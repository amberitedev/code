# src/infrastructure

Infrastructure contains concrete implementations used by the application layer: SQLite repositories, auth validation, process spawning, event broadcasting, Minecraft downloads/installers, Modrinth calls, and server file helpers.

## Mental Model

Application services decide what should happen. Infrastructure performs the I/O needed to make it happen.

The main systems are:

- `db/` persists Core state in SQLite and implements repository-style ports.
- `process/` launches server processes and owns the per-instance actor.
- `minecraft/` installs servers, detects Java, talks to Modrinth, handles `.mrpack` files, and edits server files.
- `auth/` validates JWTs against the paired Core auth configuration.
- `events.rs` broadcasts instance output and status events to HTTP/WebSocket/SSE consumers.

Process lifecycle is actor-based. Starting an instance creates an in-memory handle. Stop/kill/command requests are sent to the actor, and the actor updates durable status as the process changes.

Minecraft installation writes launch metadata that later start logic reads back instead of re-deriving how the server should be launched.

## File Relationships

- `db/AGENTS.md` covers SQLite connection and repositories.
- `events.rs` is consumed by application services and presentation streaming handlers.
- `auth/jwks.rs` is consumed by `presentation/extractors.rs`.
- `process/std_spawner.rs` is the production process spawner.
- `process/mock_spawner.rs` is used by tests.
- `process/instance_actor.rs` owns the running process loop and status/event updates.
- `minecraft/server_jar.rs` routes installation by loader type.
- `minecraft/installer.rs` handles installer-based loaders.
- `minecraft/shared/` handles shared Vanilla/Fabric/Quilt assets.
- `minecraft/java.rs` detects Java and maps Minecraft versions to Java versions.
- `minecraft/modrinth_api.rs` talks to Modrinth.
- `minecraft/mrpack.rs` parses and installs `.mrpack` archives.
- `minecraft/server_properties.rs` reads and patches `server.properties`.

## Related Areas

- Application workflows that call infrastructure live in `src/application/`.
- HTTP handlers that expose infrastructure-backed behavior live in `src/presentation/`.
- Schema for persistent infrastructure data lives in `migrations/`.
