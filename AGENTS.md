# Amberite — Monorepo

An open-source project that lets users self-host a Rust-based Core server manager and control it through a desktop app forked from Modrinth.

This repository is a fork of `modrinth/code`. A Git remote named `upstream` is already configured for that upstream repository and can be used when a task involves restoring or reverting code from upstream instead of from local `HEAD`.

## Current release stage

Amberite is in the final push toward its Version 1.0 release. Before doing product work, read these files together so implementation decisions match the intended product and current release state:

- `PROJECT.md` — product definition and boundaries.
- `feature-list.md` — canonical feature and release specification.
- `TODO.md` — current Version 1.0 release blockers.

do not modify `TODO.md` when finishing a task unless user asks you can only checak off stuff in `TODO.md` if user says that thare done.

---

## Repository context — read this first

read the corresponding `AGENTS.md` for the area you're working on


| Subproject     | AGENTS.md                         | Description                                                        |
| -------------- | --------------------------------- | ------------------------------------------------------------------ |
| `frontend`     | `apps/frontend/AGENTS.md`         | Modrinth website (Nuxt 3, upstream not yet modified)               |
| `app-frontend` | `apps/app-frontend/AGENTS.md`     | Desktop app frontend (Vue 3, Modrinth fork)                        |
| `app`          | `apps/app/AGENTS.md`              | Desktop app shell (Tauri)                                          |
| `core`         | `apps/core/AGENTS.md`             | Copal server manager (Rust)                                        |
| `amberite-api` | `packages/amberite-api/AGENTS.md` | Shared communication library for app, core, and Convex integration |




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

Run from the repo root unless noted. Do not run build or dev commands unless explicitly asked. The `$implement` workflow is an exception: it may run the dev commands needed to implement and test its production slice, but must not run builds unless explicitly asked.

### Main dev servers


| Command               | What it starts                                              |
| --------------------- | ----------------------------------------------------------- |
| `pnpm app:dev`        | Desktop app (Tauri + Vite on port **1420**)                 |
| `pnpm app:dev:no-hmr` | Same, without HMR (`tauri.no-hmr.conf.json`)                |
| `pnpm core:dev`       | Copal (`cargo run` in `apps/core/`, default port **16662**) |
| `pnpm web:dev`        | Modrinth website (`apps/frontend/`)                         |
| `pnpm docs:dev`       | Docs site (`apps/docs/`)                                    |


Convex is standalone in `convex/`. Whenever code under `convex/` changes outside a worktree, push the update with `pnpm exec convex dev --once --tail-logs disable` before calling the task done. Never push or deploy Convex code from a worktree; follow the worktree development-state instructions below instead. Do not start long-running Convex watchers unless explicitly asked or running the `$implement` workflow.

### Worktree development state

Use these commands during `$implement` and other explicitly authorized end-to-end testing:

- When code under `convex/` changes, run the worktree-local Convex developer deployment so the changed backend executes during testing. Use `pnpm convex:dev:local -- accounts` for account/auth flows or `pnpm convex:dev:local -- group` for group/Core-linked flows. Keep it running through computer-use testing. Never push or deploy it from the worktree.
- When `convex/` does not change, keep the existing deployment. Do not switch to local Convex merely because a feature calls the backend.
- Preserve local Convex data while iterating. Reset only when a clean baseline is needed with `pnpm convex:dev:reset -- accounts` or `pnpm convex:dev:reset -- group`. Use `pnpm convex:dev:status` to inspect it and `pnpm convex:dev:stop` when it is no longer needed.
- Use the `owner`, `friend`, and `other` accounts for distinct permissions and views. List running apps with `pnpm app:dev:list`, switch an app with `pnpm app:dev:account -- <app-id> <username>`, and focus it with `pnpm app:dev:focus -- <app-id>`. Start one with `pnpm app:dev -- <username>` only when the active workflow permits starting dev commands.
- For Core state, run `pnpm core:setup -- list`, then `pnpm core:setup -- <name>` while the worktree's Core is running. Add an idempotent API-based setup only when repeatedly reaching the required state through the UI is impractical.
- Use existing fixtures and mocks for isolated automated tests. The final computer-use pass must exercise the real production path implemented by the slice.



### Lint, test, build


| Command                   | Scope                           |
| ------------------------- | ------------------------------- |
| `pnpm prepr`              | Format/lint across the monorepo |
| `pnpm prepr:frontend`     | Frontend + app-frontend only    |
| `pnpm prepr:frontend:app` | App frontend only               |
| `pnpm lint` / `pnpm fix`  | Turbo lint/fix                  |
| `pnpm test`               | Turbo test                      |
| `pnpm ci`                 | Lint + test                     |
| `pnpm build`              | Turbo build (all packages)      |
| `pnpm storybook`          | `@modrinth/ui` Storybook        |


Pre-PR commands run from the root folder before opening a pull request. Do not run `prepr` commands after each user prompt. Only run them when asked. If the user indicates they are about to create a pull request, ask whether they want the relevant `prepr` command run.

Core Rust checks run from `apps/core/` (isolated Cargo workspace): `cargo check`, `cargo test`, `cargo run -- check`.

`@amberite/amberite-api` tests: `pnpm --filter @amberite/amberite-api test`.

---


## General rules

Notion plans are user-facing planning documents stored as Notion pages. Create them with the Notion connector, lead with the outcome and next action, keep them short, and place supporting detail in expandable sections.

- Use `@` alias for `src/` in imports.
- Named exports over default exports.
- Required config env vars must error clearly if missing or invalid. No fallbacks.
- Do not modify `packages/app-lib` unless explicitly asked.
- Do not read plans in `.plan/` for orientation; only read a specific plan if the user explicitly asks.
