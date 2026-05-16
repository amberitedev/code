# Amberite — Monorepo

Free, self-hosted platform for playing modded Minecraft with friends. One person
runs the Core (server manager); everyone else installs the App and gets synced
automatically. The App is a full fork of the Modrinth desktop app. Core is custom
Rust with zero Modrinth code. License: AGPL-3.

GitHub: `amberitedev/code` (forked from `modrinth/code`)

---

## Context Loading — READ THIS FIRST

**DO NOT scan, explore, or glob the codebase to build a project overview.** Context is pre-built per subproject.

| Subproject     | AGENTS.md                     | What it covers                              |
| -------------- | ----------------------------- | ------------------------------------------- |
| `app-frontend` | `apps/app-frontend/AGENTS.md` | Desktop app frontend (Vue 3, Modrinth fork) |
| `app`          | `apps/app/AGENTS.md`          | Desktop app shell (Tauri)                   |
| `core`         | `apps/core/AGENTS.md`         | Amberite Core - server manager (Rust)       |

Remember to update an AGENTS.md when:
- You added, removed, or restructured something
- You discovered a gotcha that isn't documented
- You had to figure something out that should have been cached and wasn't

Do not update for routine work. Keep the cache clean.

### Plans (`.plan/`)

Plans in `.plan/` are off-limits unless the user explicitly names a specific
plan to read or asks you to implement that plan. Read only the requested plan,
not the rest of the directory. Treat plans as planning notes, not source of
truth; they are usually at least partially wrong.

---

## Architecture

- **Monorepo tooling:** Turborepo (`turbo.jsonc`) + pnpm workspaces (`pnpm-workspace.yaml`)
- **Frontend:** Vue 3, Tailwind CSS v3
- **App shell:** Tauri (Rust)
- **Core:** Rust/Axum
- **Indentation:** Tabs everywhere, never spaces

### Packages (`packages/`)

| Package          | Description                                                |
| ---------------- | ---------------------------------------------------------- |
| `ui`             | Shared Vue component library (`@modrinth/ui`)             |
| `assets`         | Styling and auto-generated icons                           |
| `api-client`     | API client (Nuxt, Tauri, Node/browser)                    |
| `app-lib`        | Shared app library — **do not modify unless explicitly asked** |
| `tooling-config` | ESLint, Prettier, TypeScript configs                      |

---
#### Dev Commands

| Task               | Command         | Directory    |
| ------------------ | --------------- | ------------ |
| App frontend dev   | `pnpm app:dev`  | root         |
| App frontend build | `pnpm build`    | `apps/app/`  |
| Storybook          | `pnpm story`    | root         |
| Core dev           | `cargo run`     | `apps/core/` |
| Core tests         | `cargo test`    | `apps/core/` |

`.env` files are gitignored. Copy `.env.example` as template before running.
`packages/app-lib/` needs its own `.env` — copy template before running the app.

---

## Pre-PR Commands

Run from root. Only run when opening a PR — do not run after every prompt.

| Target        | Command                   |
| ------------- | ------------------------- |
| App frontend  | `pnpm prepr:frontend:app` |
| Frontend libs | `pnpm prepr:frontend:lib` |
| All frontend  | `pnpm prepr`              |

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
- Types in `@modrinth/utils` are considered highly outdated, if a component needs them, check if you can switch said component to use types from `packages/api-client`
- When provided problems, do not say "I didn't introduce these problems" (shifting the blame/effort) — just fix them.

## General Rules

- Max 200 lines per file. Ask before exceeding.
- One component per file.
- Use `@` alias for `src/` in imports.
- Named exports over default exports.
- Do not modify `packages/app-lib` unless explicitly asked.
- never read PROJECT-HUMAN-ONLY.md unless user speficly asks.

