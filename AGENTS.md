# Amberite — Agent Instructions

Amberite is a free, self-hosted platform for playing modded Minecraft with friends. One person runs the Core (server manager); everyone else installs the desktop App and is automatically synced to the right mods. The App is a full fork of the Modrinth desktop app (`modrinth/code`). The Core is fully custom Rust with zero Modrinth code. License: AGPL-3.

GitHub repo: `amberitedev/code` (fork of `modrinth/code`).

---

## Session Start

Read `.plan/active/bugs.md` at the start of each session for current open bugs.

Consult `PROJECT.md` only when asked about architecture, planning, or making significant design decisions — do not load it automatically.

Do not read `.plan/active/features.md` or `.plan/active/decisions.md` — those are superseded by `PROJECT.md`.

Each app also has its own `AGENTS.md`. Read the relevant one when working in that app.

---

## Projects

| Directory | What it is | Status |
|---|---|---|
| `apps/app/` | Desktop client — Vue 3 + Tauri (Modrinth fork + Amberite patches) | In progress |
| `apps/core/` | Core — fully custom Rust/Axum server manager | In progress |
| `apps/supabase/` | Edge Functions + Supabase setup | Partial |
| `apps/web/` | Cloudflare Pages site (amberite.dev) | Barely started |
| `apps/mod/` | Companion Minecraft mod | **Not started — do not create** |

---

## Repo Structure

This repo is a full fork of `modrinth/code`. Modrinth upstream packages live under `packages/`. All Amberite-specific code is in `apps/` or `packages/amberite-lib/` (planned).

```
amberitedev/code/
├── apps/
│   ├── app/                    — desktop client (Modrinth fork + Amberite patches)
│   │   ├── frontend/           — Vue 3 + Tailwind UI
│   │   ├── backend/            — Amberite backend lib (future: packages/amberite-lib/)
│   │   └── tauri/              — Tauri desktop shell (READ-ONLY)
│   ├── core/                   — fully custom Rust/Axum Core (zero Modrinth code)
│   │   ├── src/
│   │   │   ├── domain/
│   │   │   ├── application/
│   │   │   ├── infrastructure/
│   │   │   └── presentation/
│   │   └── migrations/
│   ├── supabase/
│   │   └── functions/
│   │       ├── microsoft-auth/
│   │       ├── create-invite-link/
│   │       └── join-group/
│   └── web/
│       └── src/
│           ├── pages/
│           └── components/
├── packages/
│   ├── amberite-lib/           — PLANNED: Amberite backend lib (currently apps/app/backend/)
│   ├── app-lib/                — Modrinth theseus (upstream)
│   ├── ui/                     — shared Vue 3 + Tailwind components (upstream)
│   ├── utils/                  — shared utilities (upstream)
│   ├── config/                 — Tailwind, ESLint, tsconfig base (upstream)
│   ├── api-client/             — Modrinth API client (upstream)
│   └── ...                     — other upstream Modrinth packages
└── scripts/
```

---

## Dev Environment

**WSL** — Core development. Clone the repo inside WSL's own filesystem (not `/mnt/c/`). All Rust compilation and Core testing happens here.

**Windows** — App development. Vue frontend + Tauri on Windows. App connects over the network to Core running in WSL.

| Task | Command | Directory |
|---|---|---|
| Frontend dev server | `pnpm dev` | `apps/app/` |
| Frontend build | `pnpm build` | `apps/app/` |
| Core | `cargo run` | `apps/core/` |
| Core tests | `cargo test` | `apps/core/` |

---

## Rules

- Never overwrite existing files — always edit them.
- Max 200 lines per file (hard rule). Ask before exceeding.
- One component per file (single responsibility).
- All `.env` files are gitignored. Use `.env.example` as template.
- Tauri APIs are mocked in dev — do not import real `@tauri-apps/*` without mocking.
- Use `@` alias for `src/` in imports.
- `apps/app/tauri/` is read-only — do not modify Tauri shell code.
- `packages/amberite-lib/` never modifies theseus/Modrinth internals. Patch in local copies marked `// AMBERITE PATCH`.
- `apps/mod/` does not exist yet — do not create it.

---

## Memory Tracking

Track all features, plans, decisions, and issues the user mentions throughout the session. At session end, when finished building/planning, or on request — update `PROJECT.md` and `.plan/active/bugs.md` using `/feature-memory`.
