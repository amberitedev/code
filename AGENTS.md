# Amberite

Amberite is a server management web app that runs via Tauri and manages Amberite Core. The goal is to make playing modded Minecraft with friends seamless by adding client-to-client sync and core-to-client sync to keep all mods synced across server and client.

## Session Start

Read these at the start of every session:
1. `PROJECT.md` — Full project vision
2. `.plan/active/features.md` — Planned features
3. `.plan/active/bugs.md` — Known bugs
4. `.plan/active/decisions.md` — Architectural decisions

## Projects

| Directory | What it is | State |
|-----------|------------|-------|
| `apps/app/` | Main app — Vue 3 frontend + custom Rust backend + Tauri | Stated implemented |
| `apps/core/` | Amberite Core — Rust/Axum server management | Mostly implemented |
| `apps/supabase/` | Supabase Edge Functions + setup docs | Setup files only |
| `apps/web/` | Cloudflare Pages site (amberite.dev/dashboard) — lighter dashboard | Barely implemented |

Each project has its own AGENTS.md. Read the relevant one for your task.

## Project Structure

```
amberite/
├── apps/
│   ├── app/              # Main app
│   │   ├── frontend/    # Vue 3 + Tailwind
│   │   ├── backend/     # Custom Rust/Axum backend (overrides core)
│   │   └── tauri/       # Tauri desktop shell (Rust + Vue 3)
│   ├── core/            # Amberite Core — Rust/Axum server management
│   │   ├── src/
│   │   │   ├── domain/           # Business logic, entities
│   │   │   ├── application/      # Services, orchestration
│   │   │   ├── infrastructure/  # SQLite, processes, networking
│   │   │   └── presentation/    # HTTP handlers, WebSocket
│   │   └── migrations/
│   ├── supabase/        # Edge functions + setup docs
│   │   └── functions/
│   └── web/             # Cloudflare Pages site
│       └── src/
├── scripts/
└── AGENTS.md
```

## Tech Stack

Frontend: Vue 3, Tailwind CSS, TypeScript, Vite, Pinia, Vue Router, Vue Query
Backend: Rust, Axum
Desktop: Tauri (Rust + Vue 3)

Add later once someone knows.

## Dev Environment

Add later once someone knows. This might change.

## Dev Commands

Frontend (in `apps/app/frontend/`):
- `pnpm dev` — Start dev server
- `pnpm build` — Type-check + build

Backend (in `apps/app/backend/`):
- `cargo run` — Start backend server
- `cargo build` — Build backend
- `cargo test` — Run tests

## Rules

- Never overwrite existing files — always edit them instead
- Max 200 lines per file (hard rule)
- One component per file (single responsibility)
- If you need to exceed 200 lines, ask for permission first
- All `.env` files are gitignored. Use `.env.example` as template
- Tauri APIs are mocked in dev — do not import real `@tauri-apps/*` without mocking
- Use `@` alias for `src/` in imports
- `apps/app/tauri/` is read-only — do not modify Tauri app code

## When to Refresh

If you make significant structural changes, tell the user to update relevant AGENTS.md files.

## Memory Tracking

Track all features, plans, decisions, and ideas the user mentions throughout the session. At session end, when finished building/planning, or on request — update `.plan/active/` files using `/feature-memory`. Memory files are the only files you may update in plan mode.