# Amberite

Amberite is a server management web app that runs via Tauri and manages Amberite Core, which is the actual server manager.

## Projects

| Directory | What it is | State |
|-----------|------------|-------|
| `apps/app/` | Main app — frontend + custom backend + Tauri | Implemented |
| `apps/core/` | Amberite Core — actual server management logic | Implemented |
| `apps/supabase/` | Supabase — auth, friends, groups, registration | Implemented |
| `apps/web/` | Cloudflare Pages site (amberite.dev) | Partially implemented |

Each project has its own AGENTS.md. Read the relevant one for your task.

## Project Structure

```
amberite/
├── apps/
│   ├── app/              # Main app
│   │   ├── frontend/    # Vue 3 + Tailwind
│   │   ├── backend/     # Custom Rust/Axum backend (overrides core)
│   │   └── tauri/       # Tauri desktop shell (read-only)
│   ├── core/            # Amberite Core — server management
│   ├── supabase/        # Supabase — auth, friends, groups
│   └── web/             # Cloudflare Pages site
├── scripts/
└── AGENTS.md
```

## Tech Stack

Frontend: Vue 3, Tailwind CSS, TypeScript, Vite, Pinia, Vue Router, Vue Query
Backend: Rust, Axum
Desktop: Tauri

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

- Max 200 lines per file (hard rule)
- One component per file (single responsibility)
- If you need to exceed 200 lines, ask for permission first
- All `.env` files are gitignored. Use `.env.example` as template
- Tauri APIs are mocked in dev — do not import real `@tauri-apps/*` without mocking
- Use `@` alias for `src/` in imports
- `apps/app/tauri/` is read-only — do not modify Tauri app code

## When to Refresh

If you make significant structural changes, tell the user to update relevant AGENTS.md files.