# Amberite

Amberite is a self-hosted modded Minecraft launcher and server manager for private friend groups. A
Rust server called Copal runs on a computer controlled by the group, and the desktop app is built on
top of the Modrinth App.

You can think of Amberite as Modrinth with private, self-hosted group servers built into it. Owners
and Admins manage the server; everyone else installs, updates, and plays.

## What makes Amberite special?

This project is very new and has not yet been deployed or tested anywhere, but there are some things we should never compromise on.

### 1. Easy to use

Amberite makes everything feel simple and easy. The technical stuff is hidden behind advanced settings. If an average non-technical user can’t do something, it’s too complicated.

### 2. Self-hosted Core

Copal/Core is Amberite’s self-hosted server manager. It is open source and free, runs on hardware controlled by the group, and fully replaces Modrinth Hosting.

### 3. Modrinth handles public content

Modrinth handles public content (mods, modpacks, resource packs, etc.). Amberite uses that ecosystem and does not intend to replace it.

### 4. The App builds on Modrinth

The App keeps the full Modrinth experience. Users can continue using it as their main launcher (browsing content, managing instances, installing modpacks and mods, etc.), with Amberite’s features added on top.

Amberite ports Modrinth’s social backend from `apps/labrinth` (friends, profiles, sharing, etc.) to Convex while keeping the same schemas and contracts. Amberite runs this backend itself, with relevant upstream changes carried over.

### 5. Minecraft-first identity

A verified Minecraft account is the basis of your Amberite identity and is used to sign in. Modrinth accounts are linked only for Modrinth features.

## A note from Ilai

I like ambitious ideas, simple systems, and software that feels obvious. Do not preserve complexity just because it already exists. Do not introduce machinery because it looks architecturally impressive. Understand the real constraint, then fight for the smallest model that makes the correct behavior unsurprising.

Channel both "measure twice, cut once" and "yagni". Fight scope creep. Try to honor the dev's intent in both a minimal and realistic fashion.

The rest of this document is meant to help you navigate the codebase and make changes effectively. Think of these instructions less as "hard rules", more as "good defaults". The developer's preferences should be able to override anything here.

## Hit every surface

The most common frontend defect is a change that works on the side that was tested and is missing
somewhere else. Before calling frontend work done, check which of these apply:

- **Permissions.** Shared and administrative features need an explicit permission. Add it to the
  permission model, decide which roles receive it, enforce it outside the UI, and reflect it in the
  UI. Ask the developer when the intended access is unclear.
- **Both ends.** Check every side of a feature, not only the side you changed.
- **API client.** Any communication between clients (app) and backends (core, convex) is typed in
  `packages/api-client`.
- **Reverse states.** If you added a way in, add the way out and the way to see it. A one-way door is
  a bug.
- **Shared UI.** Changes to inherited Modrinth UI or `packages/ui` can affect screens outside the one
  you changed. Check its consumers before treating it as local.

## Dev servers

- `vp i` installs dependencies. Worktrees get this from the `t3.json` setup script, which also copies
  the primary checkout's `.data/`. If module resolution or development data looks broken, setup
  probably did not run.
- `vp run dev` starts Convex, one Core, and the App scenarios selected by `dev.json`. Pass scenario
  numbers to run several isolated Apps against the same backends: `vp run dev 1 2 3`.
- The primary checkout uses the cloud Convex development deployment. `vp run dev` watches and pushes
  Convex changes there. Every linked worktree uses its own local Convex deployment and must never
  push Convex changes to the cloud; this is determined by the checkout, not the branch name.
- Worktree state lives in that worktree's gitignored `.data/`. Convex and Core are shared;
  `scenarios/<number>/` is the complete persistent state of one App installation. Do not point a
  worktree at another checkout's live state.
- Ports derive from the worktree path and stay stable across restarts when available. Read the real
  ports from the `[dev-runner]` output or `.data/runtime.json`, because occupied ports shift.
- `vp run dev:app`, `vp run dev:core`, and `vp run dev:convex` start only that part of the
  environment.
- If you start a process, record its PID and stop exactly that process. Never kill by a broad process
  name or path; several worktrees may be running at once.

Full command, state, Convex, and port behavior: `docs/internals/scripts.md`.

## Test data

An empty environment is a bad test. Worktrees get an isolated copy of the primary checkout's
`.data/` instead of pointing at live state:

- `.data/convex` and `.data/core` are shared by every App in the worktree.
  `.data/scenarios/<number>` contains the complete local state of one App installation.
- Run `vp run dev 1 2 3` to launch several Apps as different fake accounts against the same Convex and
  Core. Each scenario keeps its own database, settings, Minecraft instances, credentials, and
  WebView state.
- Scenarios `1` through `4` are the default test set. A new positive number creates another isolated
  App state and fake account when the task needs one.
- Treat Convex, Core, and the App scenarios as one dataset. Copy them together when replacing the
  baseline; do not create Core-only scenarios.
- Stop the affected process before editing or copying SQLite state. A live file copy is not safe
  unless its `-wal` and `-shm` files are copied with it.
- Copy data into the worktree, never symlink it to another checkout. Test data flows into the
  sandbox, never back out.

## Verifying

- Use the smallest proof that the change works: `vp test run <files>` for tests you touched, targeted
  lint, and typecheck only for the workspace you changed.
- **Do not run repo-wide checks.** No `vp check`, `vp run -r test`, or `vp run -r typecheck` unless
  the developer asks. CI owns the full suite.
- Do not add backend tests by default. Add them when the developer asks or when the behavior is
  stable enough that a focused regression test is useful.
- Upon request, user-visible App changes get one integrated pass against the real App and real
  backends. Use the browser bridge to access the App for normal workflows, and its desktop window
  only when behavior depends on Tauri or the native shell. The primary agent does this once after
  integrating. Subagents do not start their own dev servers. Ask permission before computer use or
  opening a browser.

## Pull requests

- Never create or publish a pull request unless the developer explicitly asks.
- Use conventional titles in plain language.
- Keep one concern per pull request.
- Start the body with the problem, then explain the solution and focused verification.
- UI changes need before-and-after images. Motion or timing changes need a short video.
- Do not include unrelated working-tree changes.

## Where code lives

- `apps/app-frontend` - Vue desktop product UI and inherited Modrinth launcher pages.
- `apps/app` - Tauri shell, native commands, capabilities, and process integration.
- `apps/core` - Copal, the Rust server manager.
- `apps/frontend` - Website and inherited Modrinth web surface.
- `convex` - durable Amberite identity, social, group, and cloud state.
- `apps/realtime` - short-lived presence through Cloudflare Workers and Durable Objects.
- `packages/ui` - shared Modrinth and Amberite UI.
- `packages/api-client` - typed Modrinth API client.
- `packages/app-lib` - inherited launcher and platform library. Do not modify it unless explicitly
  asked.
- `docs/internals` - architecture, glossary, and focused maintainer documentation.

## Taste

- Complexity belongs at the adapter boundary. Backend-facing workflows stay in
  `@modrinth/api-client`; UI stays dumb.
- Preserve upstream behavior until Amberite has a product reason to differ.
- Inferred types over annotations. Never use TypeScript `any`; use `unknown` and narrow it safely.
- Comments describe how a thing is used or why a constraint exists. Do not narrate ordinary code.
- If a rule here fights the task, say so clearly and get developer approval before breaking it.

## Additional tips

- Don't verify with browsers or computer use unless the user explicitly agrees or requests it.
