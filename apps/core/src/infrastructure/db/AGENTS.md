# src/infrastructure/db

SQLite persistence for Core.

## Mental Model

The database is opened once and stored in `AppState`. Repositories here implement port traits from `src/ports/` for durable entities such as instances, Java installs, and modpack manifests.

Not every table has a repository. Feature-specific tables may be queried directly from services or handlers with `state.pool` when no port abstraction exists yet.

Schema changes live in `apps/core/migrations/`. Repository changes and migration changes usually happen together.

## File Relationships

- `mod.rs` creates the SQLite pool and configures the connection.
- `instance_repo.rs` implements instance persistence for application services.
- `java_repo.rs` persists detected Java installations.
- `modpack_repo.rs` persists installed modpack manifests.
- `src/ports/` defines the traits these repositories implement.
- `src/application/` calls repositories through `AppState` ports or uses `state.pool` for feature-specific SQL.
- `apps/core/migrations/` defines the schema these files expect.

## Related Areas

- Add or change schema: `apps/core/migrations/AGENTS.md`.
- Add or change service behavior using DB data: `apps/core/src/application/AGENTS.md`.
- Add or change HTTP exposure of DB data: `apps/core/src/presentation/AGENTS.md`.
