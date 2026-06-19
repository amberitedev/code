# Amberite — Monorepo

An open-source project that lets users self-host a Rust-based Core server manager and control it through a desktop app forked from Modrinth.

This repository is a fork of `modrinth/code`. A Git remote named `upstream` is already configured for that upstream repository and can be used when a task involves restoring or reverting code from upstream instead of from local `HEAD`.
---

## Context Loading — READ THIS FIRST

**DO NOT scan, explore, or glob the codebase to build a project overview.** Context is pre-built per subproject.

| Subproject        | AGENTS.md                          | Description                                                        |
| ----------------- | ---------------------------------- | ------------------------------------------------------------------ |
| `frontend`        | `apps/frontend/AGENTS.md`          | Modrinth website (Nuxt 3, upstream not yet modified)               |
| `app-frontend`    | `apps/app-frontend/AGENTS.md`      | Desktop app frontend (Vue 3, Modrinth fork)                        |
| `app`             | `apps/app/AGENTS.md`               | Desktop app shell (Tauri)                                          |
| `core`            | `apps/core/AGENTS.md`              | Copal server manager (Rust)                                |
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

Run from the repo root unless noted. dont run build or dev commands, except Convex deploy/codegen pushes required after editing `convex/`.

### Main dev servers

| Command | What it starts |
| ------- | -------------- |
| `pnpm app:dev` | Desktop app (Tauri + Vite on port **1420**) |
| `pnpm app:dev:no-hmr` | Same, without HMR (`tauri.no-hmr.conf.json`) |
| `pnpm core:dev` | Copal (`cargo run` in `apps/core/`, default port **16662**) |
| `pnpm web:dev` | Modrinth website (`apps/frontend/`) |
| `pnpm docs:dev` | Docs site (`apps/docs/`) |

Convex is standalone in `convex/`. Whenever code under `convex/` changes, push the update with `pnpm exec convex dev --once --tail-logs disable` before calling the task done. Do not start long-running Convex watchers unless explicitly asked.

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

Pre-PR commands run from the root folder before opening a pull request. Do not run `prepr` commands after each user prompt. Only run them when asked. If the user indicates they are about to create a pull request, ask whether they want the relevant `prepr` command run.

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

- One component per file.
- Use `@` alias for `src/` in imports.
- Named exports over default exports.
- Do not modify `packages/app-lib` unless explicitly asked.
- Never read `PROJECT-HUMAN-ONLY.md` unless the user specifically asks.
- Do not read plans in `.plan/` for orientation; only read a specific plan if the user explicitly asks.
- Always use existing: components from the `packages/ui`, text sizes, fonts, layouts, padding, and more
- Never write new ui unless seificly asked. the compenent library and ui in the desktop app has everything you need.

## Desktop App UI Rules

These rules apply to any task touching `apps/app-frontend` or app-facing UI in `packages/ui`.

- Load `apps/app-frontend/AGENTS.md` first, then `packages/ui/AGENTS.md`.
- Treat the desktop app as an extension of the existing Modrinth App UI, not a greenfield design surface.
- Do not invent new visual systems, spacing scales, font stacks, card styles, button variants, or color palettes.
- Prefer existing `@modrinth/ui` components, layouts, and patterns over custom markup.
- Before building any new UI, search for an existing component or layout in `packages/ui/src/components` and `packages/ui/src/layouts`.
- If an existing shared component is close, adapt or compose it instead of cloning it locally.
- Only create new UI components when the existing library truly cannot express the need, and state that reason in your final response.
- Use existing surface, text, border, and semantic color tokens. Do not hardcode hex colors, ad-hoc Tailwind colors, or alternate fonts for production UI.
- Match existing spacing and density from nearby app screens. New pages should look like they belong next to adjacent routes in `apps/app-frontend/src/pages` without visual surprise.
- If a request is specifically about desktop app UI fit-and-finish, prioritize consistency over novelty.
