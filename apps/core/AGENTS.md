# Lodestone Core

Rust/Axum backend for managing Minecraft servers — REST API on port `16662`.

## What This Is

Lodestone Core V2 manages Minecraft server instances — start/stop servers, console access, configuration, and user scripting via JavaScript macros. REST API with WebSocket console streaming.

## Context Loading

Subdirectories will have their own `AGENTS.md` files for layer-specific context. Read them on-demand based on what you're working on.

## Project Structure

```
core/
├── src/
│   ├── domain/           # Pure business logic, typestates, ports
│   ├── application/      # Services, actors, orchestration
│   ├── infrastructure/   # SQLite, Deno, processes, networking
│   ├── presentation/     # Axum routes, handlers, WebSocket
│   └── main.rs           # Entry point, wires layers
├── migrations/           # SQLx migrations
├── Cargo.toml            # Dependencies (Rust 1.88.0)
└── data.db               # SQLite database
```

## Tech Stack

- **Web Framework:** Axum 0.7 (REST + WebSocket)
- **Database:** SQLx 0.9 (SQLite with migrations)
- **Scripting:** Deno Core 0.354 (JavaScript macros)
- **Auth:** PASETO v4, Argon2 password hashing
- **Async:** Tokio 1.47
- **Serialization:** Serde, ts-rs
- **Validation:** Garde
- **Logging:** Tracing, color-eyre

## Commands

| Command | Description |
|---------|-------------|
| `cargo run -- --lodestone-path <dir>` | Start server |
| `cargo build` | Build |
| `cargo test` | Run all tests |
| `cargo test -- --nocapture` | Run tests with output |
| `cargo test <name>` | Run specific test |
| `cargo check` | Type-check |
| `cargo fmt` | Format |
| `cargo clippy` | Lint |

## Global Rules

- Rust version: 1.88.0
- SQLite with SQLx migrations (auto-applied on startup)
- `.env` files are gitignored
- Max ~200 lines per file
- One primary struct/enum per file
- Colocated tests: `#[cfg(test)] mod tests { ... }`
- Route protection via Axum Middleware/Extractors
- Explicit imports (no glob `*`)

## Architecture

Clean Architecture: **Domain** (pure logic) → **Application** (orchestration) → **Infrastructure** (implementations) → **Presentation** (HTTP API).

Key patterns: Dependency inversion via ports, Actor model for instances, Typestate for compile-time state safety.

## Supabase Notes

- **JWT Secret:** Currently using anon key for validation. For production, replace with service role key for Core's status updates to Supabase.
- **Service Role Key:** Get from Supabase Dashboard → Settings → API → Service Role Key (click to reveal)
