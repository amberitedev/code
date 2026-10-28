# Amberite

Amberite is a modded Minecraft launcher with instance sharing and self-hosted servers built in. A
Rust server called Copal runs on whatever computer or cloud machine you install it on, and the
desktop app is built on top of the Modrinth App.

You can think of Amberite as Modrinth with sharing and self-hosted servers built into it. You share
your modpack with your friends, and any of you can run it as a server and control that server from
the App. Sharing is not tied to a Core and works for everyone. There are no friend groups.

## What makes Amberite special?

This project is very new and has not yet been deployed or tested anywhere, but there are some things we should never compromise on.

### 1. Easy to use

Amberite makes everything feel simple and easy. The technical stuff is hidden behind advanced settings. If an average non-technical user can’t do something, it’s too complicated.

### 2. Self-hosted Core

Copal/Core is Amberite’s self-hosted server manager. It is open source and free, runs on hardware you control, and fully replaces Modrinth Hosting. A server can be created from an instance, so its content follows the instance without anyone managing or syncing it.

### 3. Modrinth handles public content

Modrinth handles public content (mods, modpacks, resource packs, etc.). Amberite uses that ecosystem and does not intend to replace it.

### 4. The App builds on Modrinth

The App keeps the full Modrinth experience. Users can continue using it as their main launcher (browsing content, managing instances, installing modpacks and mods, etc.), with Amberite’s features added on top.

Amberite ports Modrinth’s social backend from `apps/labrinth` (friends, profiles, sharing, etc.) to `apps/backend` (Cloudflare Workers and D1) while keeping the same schemas and contracts. Amberite runs this backend itself, with relevant upstream changes carried over.

### 5. Minecraft-first identity

A verified Minecraft account is the basis of your Amberite identity and is used to sign in. Modrinth accounts are linked only for Modrinth features.

## A note from Ilai

I like ambitious ideas, simple systems, and software that feels obvious. Do not preserve complexity just because it already exists. Do not introduce machinery because it looks architecturally impressive. Understand the real constraint, then fight for the smallest model that makes the correct behavior unsurprising.

Channel both "measure twice, cut once" and "yagni". Fight scope creep. Try to honor the dev's intent in both a minimal and realistic fashion.

The rest of this document is meant to help you navigate the codebase and make changes effectively. Think of these instructions less as "hard rules", more as "good defaults". The developer's preferences should be able to override anything here.

## Hit every surface

The most common frontend defect is a change that works on the side that was tested and is missing
somewhere else. Before calling frontend work done, check which of these apply:

- **Both ends.** Check every side of a feature, not only the side you changed.
- **API client.** Any communication between clients (app) and backends (Core, `apps/backend`) is typed in
  `packages/api-client`. Native sharing uploads and downloads go through `packages/app-lib`.
- **Reverse states.** If you added a way in, add the way out and the way to see it. A one-way door is
  a bug.
- **Shared UI.** Changes to inherited Modrinth UI or `packages/ui` can affect screens outside the one
  you changed. Check its consumers before treating it as local.

## Dev servers

- Worktree setup (`node scripts/setup-worktree.ts`, run by `t3.json`) installs dependencies and copies
  the primary checkout's test data. It takes a few minutes and is often still running when you
  start. Do not wait for it: reading and editing code needs none of it, so begin the work straight
  away. Look at `.data/setup.json` only when you first need to run something: `running` means it is
  not ready yet (never run `vp i` beside it), `failed` or a missing file means run the script again.
- `node scripts/dev-runner.ts up 1 2` starts the whole stack in the background and returns when it is ready,
  printing each App's browser bridge URL. `node scripts/dev-runner.ts status` prints the same at any time and
  `node scripts/dev-runner.ts down` stops it. The log is `.data/dev.log`. Apps started this way have no window
  on the desktop; add `--show-windows` when a person needs to see them.
- The App and Core are built once per distinct set of Rust inputs and shared by every checkout. A
  checkout whose Rust matches something already built is ready in about 30 seconds and compiles
  nothing. Otherwise the start compiles first, one build at a time across all checkouts: about a
  minute after a small Rust edit in a checkout that has built before. The first build in a worktree
  takes a few minutes, or compiles only what that worktree changed on a Dev Drive. `up` stops at once and prints the compiler errors when the Rust does not
  compile, so it is also the check for Rust changes. It gives up waiting after 9 minutes; run it
  again to keep waiting.
- Nothing watches Rust files. After a Rust edit, run `down` then `up`, which rebuilds because the
  inputs changed.
- `vp run dev` is the same stack in the foreground, for a person at a terminal, with App windows and the
  account sign-in website. It starts the local backend, one
  Core, and the App scenarios selected by `dev.json`. Pass scenario numbers to run several isolated
  Apps against the same backends: `vp run dev 1 2 3`.
- The backend runs locally through `wrangler dev --local`. No Cloudflare account, billing, or cloud
  deployment is used, in the primary checkout or in worktrees.
- Worktree state lives in that worktree's gitignored `.data/`. The backend and Core are shared;
  `scenarios/<number>/` is the complete persistent state of one App installation. Do not point a
  worktree at another checkout's live state.
- Ports derive from the worktree path and stay stable across restarts when available. Read the real
  ports and bridge URLs from `node scripts/dev-runner.ts status` or `.data/runtime.json`, because occupied ports
  shift and bridge ports change on every start.
- `vp run dev:app`, `vp run dev:core`, and `vp run dev:backend` start only that part of the
  environment.
- Stop what you started with `node scripts/dev-runner.ts down`, which stops only this checkout's runner. Never kill
  by a broad process name or path; several worktrees may be running at once.
- Changes that only touch the backend or Core do not need an App: `node scripts/dev-runner.ts up dev:backend` starts
  in seconds and compiles no Rust.

## Driving the App

Each running App serves itself to a browser at its bridge URL, with every native call going to the
real App. That is how agents click through it and take screenshots; no computer use is involved.

For any change a person can see, do this without asking: start the App, open it through the bridge,
use the screen you changed, and screenshot it. Scenario Apps hold only local test accounts, so their
screenshots may be uploaded for a pull request. Computer use on the real desktop still needs the
developer's go-ahead.

1. `node scripts/dev-runner.ts up 1` (or `1 2` for two accounts), then take the bridge URL from its output.
2. In T3 Code, `preview_open` that URL, then use `preview_click`, `preview_type`, and
   `preview_evaluate` to work the UI. Open one tab per App and pass its `tabId` on every call.
3. `preview_snapshot` with `save: true` writes a PNG and returns its path. Read that file to look at
   the screen. For a pull request, upload it with the `file-upload` skill and embed the URL.
4. `node scripts/dev-runner.ts down` when you are finished.

- Take the "before" screenshot before you edit; the App reloads UI changes on its own, so the
  "after" is the same tab a moment later. Motion and timing changes get a short recording
  (`preview_recording_start` / `preview_recording_stop`) instead.
- The first load of a tab takes around 20 seconds and shows a blank page meanwhile. Wait for the
  sidebar before judging or screenshotting.
- A second tab that T3 Code is not showing does not lay out: `innerText` reads as empty and
  `preview_click` on its toasts and modals times out. Read `textContent` and click through the DOM
  with `preview_evaluate` there.

- Each App signs itself in at startup as its scenario account (`scenario_<number>`), so screens
  behind an account work straight away. Nothing needs signing in by hand.
- Calls made through `preview_evaluate` are cut off after 15 seconds. Start long work and poll for
  the result instead of awaiting it.
- Open only bridge URLs in the preview. The raw App URL has no native calls, and the account sign-in
  website hangs the preview tools.
- Only the native window shows window chrome, system dialogs, and drag and drop from the desktop.
  Those need a person or computer use, and an App started with `--show-windows`.

Full command, state, and port behavior: `docs/internals/scripts.md`.

## Test data

An empty environment is a bad test. Worktrees get an isolated copy of the primary checkout's
`.data/` instead of pointing at live state:

- `.data/backend` (accounts and sharing metadata) and `.data/core` (servers and the shared files it
  stores) are shared by every App in the worktree. `.data/scenarios/<number>` contains the complete
  local state of one App installation.
- Run `vp run dev 1 2 3` to launch several Apps as different fake accounts against the same backend and
  Core. Each scenario keeps its own database, settings, Minecraft instances, credentials, and
  WebView state.
- Scenarios `1` through `4` are the default test set. A new positive number creates another isolated
  App state and fake account when the task needs one.
- Treat the backend, Core, and the App scenarios as one dataset. Copy them together when replacing the
  baseline; do not create Core-only scenarios.
- Stop the affected process before editing or copying SQLite state. A live file copy is not safe
  unless its `-wal` and `-shm` files are copied with it.
- Copy data into the worktree, never symlink it to another checkout. Test data flows into the
  sandbox, never back out.

## Verifying

- Use the smallest proof that the change works: `vp test run <files>` for tests you touched, targeted
  lint, and typecheck only for the workspace you changed.
- You usually do not need to build or compile. CI checks every push, and `vp run dev` compiles what
  it runs. When you do need the compiler, such as for a large Rust change, run `cargo check` scoped
  to the crate you changed (`-p <crate>` or that crate's directory). Release builds
  (`cargo build --release`, `vp build`) and app bundles are slow and rarely tell you anything a check
  would not.
- You usually do not need repo-wide checks (`vp check`, `vp run -r test`, `vp run -r typecheck`).
  CI runs the full suite. Run one when your change really reaches across the repo, and prefer the
  narrowest command that covers it.
- Do not add backend tests by default. Add them when the developer asks or when the behavior is
  stable enough that a focused regression test is useful.
- User-visible App changes get one pass against the real App through the browser bridge (see
  "Driving the App"). The primary agent does this once after integrating; subagents do not start
  their own dev stacks. Ask before computer use.

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
- `apps/core/src/store` - the store: a second `copal` process that holds encrypted shared files for
  the pool. The Core forwards `/store/` to it unread; it has no database or keys and checks the
  backend's tickets itself. Design in `docs/internals/pool.md`.
- `apps/frontend` - Website and inherited Modrinth web surface. Not in use; it will be reverted to
  upstream.
- `apps/backend` - accounts, friends, presence, and sharing metadata on Cloudflare Workers, D1, and
  R2. Shared files live on Cores, never on Cloudflare.
- `packages/ui` - shared Modrinth and Amberite UI.
- `packages/api-client` - typed client for Modrinth, the backend, and Core.
- `packages/app-lib` - inherited launcher and platform library.
- `docs/internals` - `plan.md` holds product goals and decisions; `scripts.md` covers dev commands;
  `ui.md` holds the rules for how the App's UI behaves.

## Work tracking

Work lives in Linear, team `AMB`. Labels, statuses, the issue template, and Linear operations are in
`docs/agents/issue-tracker.md`. `/start` runs one session on the next issue, `/triage` sorts issues,
and `/to-tickets` turns a plan into issues.

## Taste

- Complexity belongs at the adapter boundary. Backend-facing workflows stay in
  `@modrinth/api-client`; UI stays dumb.
- Preserve upstream behavior until Amberite has a product reason to differ.
- Inferred types over annotations. Never use TypeScript `any`; use `unknown` and narrow it safely.
- Comments describe how a thing is used or why a constraint exists. Do not narrate ordinary code.
- If a rule here fights the task, say so clearly and get developer approval before breaking it.
