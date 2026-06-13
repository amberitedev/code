# migrations

SQLite schema migrations for Core.

## Mental Model

Migrations define the database shape expected by repository code and any feature-specific raw SQL. SQLx embeds migrations at compile time, and the database records which migration versions have already been applied.

## Rules

- Do not edit an existing migration that may have been applied.
- Add a new migration using the next unused number.
- Migration numbers must be unique.
- After adding, renaming, or editing a migration, run from `apps/core/`:

```bash
cargo clean -p copal
```

## Related Areas

- Repository implementations live in `apps/core/src/infrastructure/db/`.
- Services may use `state.pool` directly for feature-specific tables without repositories.
- HTTP handlers should not define schema behavior directly.
