# Amberite Core

Rust/Axum REST API on **port 16662** — manages Minecraft server instances for the Amberite platform.

## Commands

| Command | Effect |
|---------|--------|
| `cargo run` | Start server (same as `cargo run -- run`) |
| `cargo run -- check` | Validate config + DB, print paired status, exit |
| `cargo run -- migrate` | Apply pending migrations, exit |
| `cargo run -- version` | Print `amberite-core 0.1.0`, exit |
| `cargo run -- reset-pairing` | Clear `core_config`, exit — next `run` shows new pairing code |
| `cargo test --tests` | Run all 112 integration + unit tests (use `--tests` on Windows) |
| `cargo test --tests -- --nocapture` | Tests with stdout |
| `cargo test --tests --test <name>` | Run a single test file |
| `cargo build --release` | Release binary |

## Key Environment Variables

| Variable | Default | Purpose |
|----------|---------|---------|
| `AMBERITE_DATA_DIR` | `~/.amberite` | Root data directory |
| `PORT` | `16662` | HTTP listen port |
| `ALLOWED_ORIGIN` | `https://amberite.dev` | CORS origin |
| `AMBERITE_DEV` | `false` | Bypass JWT (`true` = dev mode) |
| `SUPABASE_URL` | — | Set after pairing; read from DB at runtime |
| `RUST_LOG` | — | Log filter (e.g. `amberite_core=debug`) |

Copy `.env.example` → `.env` before first run.

## Architecture (Clean Architecture)

```
domain/           pure types, no I/O
ports/            trait definitions (interfaces)
application/      services — orchestrate domain + ports
infrastructure/   concrete implementations of ports
presentation/     Axum routes, handlers, extractors
```

Each layer only imports layers above it in this list.
See each subdirectory's `AGENTS.md` for details.

## Docker

```bash
# Build image
docker build -t amberite-core .

# Start with compose (requires .env)
docker-compose up -d

# Health check (uses `amberite-core check` internally)
docker-compose ps
```

## First-Run Pairing

On startup (unpaired), Core prints a 6-digit code in the terminal.  
The App sends `POST /setup` with that code + Supabase URL to complete pairing.  
After pairing, all protected routes require a valid Supabase JWT.

## Security Notes

- **SEC-01 FIXED** — pairing brute-force: 429 after 5 wrong attempts (`AtomicU32` in `AppState`)
- **SEC-02 FIXED** — WS tickets GC'd every 5 min via `gc_ws_tickets` task in `main.rs`
- **SEC-04 FIXED** — `DELETE /instances/<non-uuid>` returns 400 (UUID parsed upfront)
- **ARCH-01–07** — several handlers bypass `InstanceStore`/`ModpackStore` with raw `sqlx::query`

## Test Suite

112 passing tests, 0 ignored.

```
tests/
  common/mod.rs       — TestApp (spawn/spawn_with_mock/spawn_paired/spawn_prod_unpaired), fixtures
  health.rs           — 3 tests
  setup.rs            — 5 tests  (full pairing flow including correct-code end-to-end)
  instances.rs        — 9 tests
  logs.rs             — 11 tests
  security.rs         — 9 tests
  lifecycle.rs        — 11 tests (MockSpawner — start/stop/kill/command)
  edge_cases.rs       — 11 tests
  instance_control.rs — 12 tests
  mods.rs             — 10 tests
  properties_stats.rs — 14 tests
```

`SQLX_OFFLINE=true` is set in `.cargo/config.toml` — adding a new migration requires
`cargo clean -p amberite-core` to force `sqlx::migrate!` to re-embed the files.
