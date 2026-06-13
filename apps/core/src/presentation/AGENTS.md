# src/presentation

Axum HTTP boundary for Core.

Presentation code registers routes, extracts auth/request data, calls application/API services, and maps errors into HTTP responses.

## Mental Model

Handlers should stay thin. Durable behavior belongs in `src/application`, `src/api`, or `src/infrastructure`.

Auth is handled through extractors except for endpoints that intentionally use another credential shape, such as WebSocket tickets or one-time file download tokens.

## File Relationships

- `router.rs` registers routes and middleware.
- `extractors.rs` turns HTTP auth state into typed request context.
- `error.rs` maps service/application errors into HTTP responses.
- `handlers/*.rs` translate HTTP requests into application/API calls.
- Application-backed handlers call `src/application/` services.
- Relay/message handlers call into `src/api/`.
- Streaming handlers consume `src/infrastructure/events.rs` through `AppState`.

## Handler Routing

| Feature | Handler |
| ------- | ------- |
| Health, version, Java, setup status | `handlers/diagnostics.rs`, `handlers/setup.rs` |
| Instances | `handlers/instances.rs`, `handlers/instance_control.rs` |
| Console, SSE, ws-token | `handlers/console.rs` |
| Mods and modpacks | `handlers/mods.rs`, `handlers/modpack.rs` |
| Logs, files, properties, stats | `handlers/logs.rs`, `handlers/fs.rs`, `handlers/properties.rs`, `handlers/stats.rs` |
| Access, players, tasks, RCON | `handlers/access.rs`, `handlers/players.rs`, `handlers/tasks.rs`, `handlers/rcon.rs` |
| Relay | `handlers/relay.rs` |
