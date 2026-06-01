# apps/core — Amberite Core

Custom Rust/Axum HTTP server that manages Minecraft server instances — start, stop, mod sync, console streaming, and first-run pairing.

## File structure

```
apps/core/
  src/
    main.rs           — CLI entry point (subcommands: run, check, migrate, version, reset-pairing)
    lib.rs            — library target (mirrors main.rs modules; exists so tests/ can import them)
    config.rs         — Config loaded from env vars at startup
    domain/           — entities, state types, events
    application/      — services, orchestration, AppState
    ports/            — async trait interfaces (stores + process spawner)
    infrastructure/   — concrete impls (SQLite repos, PTY spawner, auth, Minecraft HTTP)
    presentation/     — Axum router, handlers, extractors, error mapping
    api/              — typed message layer (kinds, routing policy, envelope, relay store, distributor)
  migrations/         — SQLx migration SQL files (compile-time embedded via proc macro)
  tests/              — integration test suite (reqwest against a real in-process server)
  scripts/            — developer diagnostic tooling (Python + PowerShell)
  Cargo.toml          — isolated workspace — NOT part of the monorepo root workspace
  .env.example        — env var template
  architecture.md     — per-file export map (kept up to date)
  docker-compose.yml  — optional Docker deployment
```

## Redirections

| Path | AGENTS.md | Summary |
|------|-----------|---------|
| `src/domain/` | `src/domain/AGENTS.md` | Entities: InstanceRecord, InstanceStatus, Event, ModpackManifest, JavaInstall |
| `src/application/` | `src/application/AGENTS.md` | Services, AppState, lifecycle orchestration |
| `src/ports/` | `src/ports/AGENTS.md` | Async trait interfaces bridging application ↔ infrastructure |
| `src/infrastructure/` | `src/infrastructure/AGENTS.md` | SQLite, auth, PTY, Minecraft HTTP clients |
| `src/presentation/` | `src/presentation/AGENTS.md` | Axum router, handlers, JWT extractor, error mapping |
| `src/api/` | `src/api/AGENTS.md` | Typed message layer: kinds, routing policy, envelope, relay store, distributor |
| `migrations/` | `migrations/AGENTS.md` | Schema history, current effective schema, migration rules |
| `tests/` | `tests/AGENTS.md` | TestApp fixture, test file breakdown, 94 integration tests |
| `scripts/` | `scripts/AGENTS.md` | Live diagnostic runner against a running Core |

## Build facts

- **Isolated workspace** — `Cargo.toml` opens with `[workspace]`; all `cargo` commands must run from `apps/core/`.
- **Dual targets** — `main.rs` + `lib.rs` declare the same module tree. The library target lets `tests/` use `use amberite_core::...`.
- **Macros disabled** — macro HTTP routes still exist, but `macro_service` returns a disabled error. Do not reintroduce embedded Deno; future plugin execution should be out-of-process.
- **SQLite bundled** — `libsqlite3-sys = { features = ["bundled"] }`; do not remove.
- **rustls ring** — `features = ["ring", "std", "tls12"]`; `aws-lc-sys` requires CMake and fails on Windows.
- **`.cargo/config.toml`** — forces `link.exe` linker on Windows (rust-lld ICEs on 238MB rusty_v8.lib); sets `SQLX_OFFLINE=true` globally.
- **`rust-toolchain.toml`** — pinned to `1.90.0`; keep Core on the pinned toolchain unless deliberately updating CI and lockfiles.

## Runtime configuration

All from env vars (`.env.example` is the template):

| Var | Default | Notes |
|-----|---------|-------|
| `PORT` | `16662` | HTTP listen port |
| `AMBERITE_DATA_DIR` | `~/.copal` | Root for all instance data and SQLite DB |
| `AMBERITE_BIND_HOST` | `127.0.0.1` | Bind host/IP; set `0.0.0.0` only for LAN/public exposure |
| `AMBERITE_PUBLIC_URL` | unset | Public URL stored with remote pairing registrations |
| `AMBERITE_DEV` | `true` in debug builds | Bypasses JWT auth; never enable in production |
| `AMBERITE_SYNC_RETAIN_COUNT` | `10` | Max `.mrpack` sync archives retained per profile |
| `ALLOWED_ORIGIN` | `https://amberite.dev` | CORS allowed origin |
| `CONVEX_URL` | unset | Convex deployment URL; when unpaired, Core uses it to register a remote pairing code |
| `RUST_LOG` | — | e.g. `amberite_core=debug` for verbose logging |

## CLI subcommands

The binary accepts subcommands via `clap`. Default (no subcommand) is `run`:

| Subcommand | What it does |
|-----------|-------------|
| `run` | Start the HTTP server (default) |
| `check` | Validate config + DB connectivity and print paired status, then exit |
| `migrate` | Apply pending SQLx migrations and exit |
| `version` | Print version string and exit |
| `reset-pairing` | Delete the `core_config` row — next `run` generates a fresh 6-digit code |

## Startup sequence (`run`)

1. Load `Config` from env / `.env`
2. Create `data_dir` if missing
3. Open SQLite pool at `{data_dir}/data.db`
4. Run pending migrations
5. Build `AppState` (loads/creates stable `core_identity`, generates pairing code + `.setup_secret` if unpaired and not dev mode)
6. Spawn `restore_instances` task (detects Java, resets transient statuses, restarts Running instances)
7. Spawn `gc_ws_tickets` task (drains expired WS tickets every 5 minutes)
8. Spawn remote pairing registration with Convex if `CONVEX_URL` is set and Core is unpaired
9. Bind Axum on `{AMBERITE_BIND_HOST}:{PORT}` and serve

## Gotchas

- **`AMBERITE_DEV` defaults on in debug builds** — if you run `cargo run` and auth seems to be bypassed, that's expected. Set `AMBERITE_DEV=false` in `.env` to test real JWT validation locally.
- **Pairing lockout**: After 5 wrong pairing-code attempts, `POST /setup` returns 429 permanently until Core is restarted. The counter resets on successful pairing.
- **`list_directory` does not canonicalize before reading entries.** A symlink inside an instance directory can list filenames outside the instance.
- **Modrinth API filenames are trusted during install/update.** `file.filename` is joined directly into `mods/` without the same sanitization used for uploads.
- **FS download tokens have no GC loop.** They expire after 5 minutes but remain in memory until used; only WS tickets are garbage-collected.
- **`.mrpack` install can panic on empty `downloads`.** It indexes `downloads[0]` without checking length, and `downloads[]` URLs are fetched server-side with no host allowlist.
- **WS ticket GC**: The `gc_ws_tickets` loop runs every 5 minutes — expired tickets are not evicted on use. The single-use enforcement happens at the handler level; GC just prevents unbounded map growth.
