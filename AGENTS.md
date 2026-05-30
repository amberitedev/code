# Amberite — Monorepo

An open-source project that lets users self-host a Rust-based Core server manager and control it through a desktop app forked from Modrinth.
---

## Context Loading — READ THIS FIRST

**DO NOT scan, explore, or glob the codebase to build a project overview.** Context is pre-built per subproject.

| Subproject        | AGENTS.md                          | Description                                                        |
| ----------------- | ---------------------------------- | ------------------------------------------------------------------ |
| `frontend`        | `apps/frontend/AGENTS.md`          | Modrinth website (Nuxt 3, upstream not yet modifyed)               |
| `app-frontend`    | `apps/app-frontend/AGENTS.md`      | Desktop app frontend (Vue 3, Modrinth fork)                        |
| `app`             | `apps/app/AGENTS.md`               | Desktop app shell (Tauri)                                          |
| `core`            | `apps/core/AGENTS.md`              | Amberite Core server manager (Rust)                                |
| `amberite-api`    | `packages/amberite-api/AGENTS.md`  | Shared communication library for app, core, and Convex integration |

## Architecture

- **Monorepo tooling:** Turborepo (`turbo.jsonc`) + pnpm workspaces (`pnpm-workspace.yaml`)
- **Frontend:** Vue 3, Tailwind CSS v3
- **App shell:** Tauri (Rust)
- **Core:** Rust/Axum
- **Indentation:** Tabs everywhere, never spaces

`.env` files are gitignored. Copy `.env.example` as template before running.
`packages/app-lib/` needs its own `.env` — copy template before running the app.

---

## Dev commands

Run from the repo root unless noted. dont run build or dev commands.

### Main dev servers

| Command | What it starts |
| ------- | -------------- |
| `pnpm app:dev` | Desktop app (Tauri + Vite on port **1420**) |
| `pnpm app:dev:no-hmr` | Same, without HMR (`tauri.no-hmr.conf.json`) |
| `pnpm core:dev` | Amberite Core (`cargo run` in `apps/core/`, default port **16662**) |
| `pnpm web:dev` | Modrinth website (`apps/frontend/`) |
| `pnpm docs:dev` | Docs site (`apps/docs/`) |

Convex is standalone in `convex/` — run directly: `pnpm dlx convex dev`

### Lint, test, build

| Command | Scope |
| ------- | ----- |
| `pnpm prepr` | Format/lint across the monorepo |
| `pnpm prepr:frontend` | Frontend + app-frontend only |
| `pnpm prepr:frontend:app` | App frontend only |
| `pnpm lint` / `pnpm fix` | Turbo lint/fix |
| `pnpm test` | Turbo test |
| `pnpm ci` | Lint + test |
| `pnpm build` | Turbo build (all packages) |
| `pnpm storybook` | `@modrinth/ui` Storybook |

Core Rust checks run from `apps/core/` (isolated Cargo workspace): `cargo check`, `cargo test`, `cargo run -- check`.

`@amberite/amberite-api` tests: `pnpm --filter @amberite/amberite-api test`.

---

## Code Guidelines

### Comments
- DO NOT use heading comments like `=== Helper methods ===`.
- Use doc comments only. Avoid inline comments unless absolutely necessary — code should be self-documenting.
- If a file exceeds 400 lines: check for a summary comment at the top first. If one exists, read that. If not, read the entire file then add a brief summary comment at the top listing the most important functions and key info with their line numbers.

## Bash Guidelines

### Output handling
- DO NOT pipe output through `head`, `tail`, `less`, or `more`
- NEVER use `| head -n X` or `| tail -n X` to truncate output
- IMPORTANT: Run commands directly without pipes when possible
- IMPORTANT: If you need to limit output, use command-specific flags (e.g. `git log -n 10` instead of `git log | head -10`)
- ALWAYS read the full output — never pipe through filters

### General
- Do not create new non-source code files (e.g. Bash scripts, SQL scripts) unless explicitly prompted to
- For Frontend, when doing lint checks, only use the `prepr` commands, do not use `typecheck` or `tsc` etc.
- Types in `@modrinth/utils` are considered highly outdated; if a component needs them, check if you can switch said component to use types from `packages/api-client`
- When provided problems, do not say "I didn't introduce these problems" (shifting the blame/effort) — just fix them.

## General rules

- Max 200 lines per file. Ask before exceeding.
- One component per file.
- Use `@` alias for `src/` in imports.
- Named exports over default exports.
- Do not modify `packages/app-lib` unless explicitly asked.
- Never read `PROJECT-HUMAN-ONLY.md` unless the user specifically asks.
- Do not read plans in `.plan/` for orientation; only read a specific plan if the user explicitly asks.
