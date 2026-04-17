# Amberite

Amberite server management app — Vue 3 frontend with Amberite Core (Rust/Axum) backend for managing Minecraft servers.

## Context Loading — READ THIS FIRST
**DO NOT scan, explore, or glob the codebase to build a project overview.** Context is pre-built per subproject.

| Subproject | AGENTS.md | What it covers |
|------------|-----------|----------------|
| **Frontend** | `src/AGENTS.md` | Vue 3 + Tailwind — components, pages, routing, API layer, dev commands |
| **Core** | `core/AGENTS.md` | Amberite Core (Rust/Axum) — endpoints, modifications, cargo commands |

1. Read the AGENTS.md for the subproject you're working on.
2. For component-specific context: read `src/components/<name>/AGENTS.md` on-demand.
3. For API layer context: read `src/api/AGENTS.md` on-demand.

Do NOT preemptively load all AGENTS.md files. Load them on-demand based on what the task requires.

## Project Structure
```
amberite/
├── core/                 # Amberite Core (Rust/Axum backend)
│   ├── src/
│   ├── migrations/
│   ├── Cargo.toml
│   └── data.db
├── panel/                # Vue + Tailwind frontend
│   ├── src/
│   ├── packages/         # Internal monorepo packages
│   ├── package.json
│   ├── vite.config.ts
│   └── tailwind.config.ts
└── AGENTS.md
```

## Tech Stack
- **Frontend:** Vue 3, Tailwind CSS, TypeScript, Vite, Pinia, Vue Router, Vue Query
- **Backend:** Amberite Core (Rust, Axum) — REST API on port `16662`
- **Bundling:** Tauri (mocked in dev via `src/mocks/tauri-apps.ts`)
- **Key libs:** `@tanstack/vue-query`, `ofetch`, `@xterm/xterm`, `three.js`/`@tresjs/core`, `apexcharts`, `@codemirror/*`, `vue-i18n`

## Dev Environment
| | URL |
|---|---|
| **Frontend** | `http://localhost:5173` |
| **Core API** | `http://localhost:16662` |

## Dev Commands
| Command | Description |
|---------|-------------|
| `pnpm dev` | Start Vite dev server (port 5173) |
| `pnpm build` | Type-check + production build |
| `pnpm preview` | Preview production build |

Core (in `core/`):
| Command | Description |
|---------|-------------|
| `cargo run` | Start Amberite Core |
| `cargo build` | Build Amberite Core |
| `cargo test` | Run tests |

## Global Rules
- All `.env` files are gitignored. Use `.env.example` as template.
- Internal packages are linked via `file:` protocol in `package.json` (`@modrinth/ui`, `@modrinth/assets`, etc.)
- Tauri APIs are mocked in dev — do not import real `@tauri-apps/*` in frontend code without mocking.
- Use `@` alias for `src/` in imports (e.g., `@/components/...`).
- Rust version: 1.88.0 (see `src-core/Cargo.toml`).
- Core uses SQLite (`dev.db`) with SQLx migrations.

## When to Refresh Summaries
If you make significant structural changes (new subprojects, new API patterns, major refactors), tell the user that the relevant AGENTS.md files should be updated.

## Modrinth OAuth Reference

**Documentation:**
- OAuth Guide: https://docs.modrinth.com/guide/oauth/
- API Overview: https://docs.modrinth.com/api/
- Source Code (scopes): https://github.com/modrinth/code/blob/main/apps/labrinth/src/models/v3/pats.rs

**Scopes for "Login with Modrinth":**
- `USER_READ` — username, avatar, user ID, role (only scope needed for auth)

**Do NOT read the full docs preemptively.** Only reference them when implementing OAuth-specific features. The docs are comprehensive but overwhelming for general development.

---

## General Rules
1. **The 200-Line Guideline:** Max file size is naturally around 200 lines. Split if larger, though functions/closures that inherently need more space are acceptable exceptions.
2. **Single Responsibility:** One primary struct/enum per file. Isolate database models, route handlers, and core logic into separate small modules.
3. **Colocated Testing:** Tests must be colocated next to the code (`#[cfg(test)] mod tests { ... }` inline).
4. **Strict Auth Encapsulation:** Route protection must use Axum Middleware/Extractors, never ad-hoc checks inside handler bodies.
5. **Explicit Exports Over Globs:** Always use explicit imports (`use crate::db::{Instance, User}`) instead of glob imports (`*`).
6. **PROJECT-PLAN.md HUMAN ONLY — NEVER READ:** This file is for human reference only. It becomes outdated quickly and contains diagrams. Always ask the user for current priorities instead of relying on this file.
7. **Plan First, Then Build:** When designing solutions, always consult industry standards or well-built open-source projects (e.g., how Discord handles invites, how GitHub handles personal access tokens, how Steam handles friend groups). If the user can't answer implementation questions, propose the industry-standard approach and explain why it's appropriate. Don't guess — reference proven patterns.
