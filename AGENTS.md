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

Amberite ports Modrinth’s social backend from upstream `apps/labrinth` (friends, profiles, accounts, etc.) to `apps/backend` (Cloudflare Workers and D1) while keeping the same schemas and contracts. Labrinth has no sharing server, so the sharing API follows the App's client instead; its intentional differences are in `docs/internals/sharing-api.md`. Amberite runs this backend itself, with relevant upstream changes carried over. Labrinth is not kept in this repo: read it from `modrinth/main`. `upstream.json` lists the upstream paths we never take (`ignore`) and the labrinth paths we port from (`watch`).

### 5. Minecraft-first identity

A verified Minecraft account is the basis of your Amberite identity and is used to sign in. Modrinth accounts are linked only for Modrinth features.

## A note from Ilai

I like ambitious ideas, simple systems, and software that feels obvious. Do not preserve complexity just because it already exists. Do not introduce machinery because it looks architecturally impressive. Understand the real constraint, then fight for the smallest model that makes the correct behavior unsurprising.

Channel both "measure twice, cut once" and "yagni". Fight scope creep. Try to honor the dev's intent in both a minimal and realistic fashion.

The rest of this document is meant to help you navigate the codebase and make changes effectively. Think of these instructions less as "hard rules", more as "good defaults". The developer's preferences should be able to override anything here.

## The three ways to hurt yourself

1. **Killing by pattern.** Never kill by process name, path, or worktree string. Several worktrees
   run their own stacks at once, and your own agent has this worktree's path in its command line.
   Stop what you started with `vp run down`.
2. **Writing to live state.** The primary checkout's `.data` is the source every new worktree copies,
   and the developer's installed App keeps its own data. Read and copy from them, but never start a
   stack against them or write to them.
3. **Reaching Cloudflare.** The backend runs only through `wrangler dev --local`. Never deploy, log
   in, or use a Cloudflare account.

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

## Dev

- Worktree setup (`t3.json`) installs dependencies and copies test data. Start working right away.
  Before you run anything, check `.data/setup.json`: `running` means wait (never run `vp i` beside
  it), unless the `pid` it names no longer exists. That, `failed`, or a missing file means run
  `vp run setup`.
- `vp run up 1 2` starts the backend, one Core, and the Apps for scenarios 1 and 2 in the
  background, and prints each App's bridge URL once it loads. `vp run status` prints them again.
  `vp run down` stops only this checkout's stack. The log is `.data/dev.log`.
- Rust builds once per distinct set of Rust inputs and is shared by every checkout. With no Rust
  changes, `up` compiles nothing. Otherwise it compiles only the repo's crates that changed, one
  build at a time across checkouts: a few minutes, more when `packages/app-lib` changed. `up`
  gives up waiting after 9 minutes; run it again to keep waiting.
- Nothing watches Rust. After a Rust edit, `vp run down` then `vp run up` rebuilds. `up` stops and
  prints compiler errors, so it is also the check for Rust changes.
- Backend-only work needs no App: `vp run up dev:backend` starts in seconds. `vp run up dev:core`
  adds the Core.
- `vp run dev` is the same stack in the foreground with App windows, for a person at a terminal.

Ports, builds, and timings: `docs/internals/scripts.md`.

## The App

Every App change gets one pass in the real App through its browser bridge. Read
`.agents/skills/test-amberite-app/SKILL.md` before you drive it. Computer use on the real desktop
needs the developer's go-ahead.

## Test data

An empty environment is a bad test. Each worktree gets a copy of the primary checkout's `.data/`:

- `.data/backend` and `.data/core` are shared by every App in the worktree.
  `.data/scenarios/<number>` is one App installation. Scenarios 1 to 4 are the default set; a new
  number makes another isolated App and account.
- The backend, Core, and scenarios are one dataset. Copy them together. Stop the process before
  copying SQLite files, or copy their `-wal` and `-shm` files too.
- Copy in, never symlink. Data flows into the sandbox, never back out.

## Verifying

- Before you push, run `vp run check:changed`. It runs CI's fast checks for the files you changed:
  lint, typos, TOML, typecheck and tests of the affected packages, and for Rust `rustfmt` and a
  build. CI still runs the full suites.
- While you iterate, use the smallest proof: `vp test run <files>`, targeted lint, and typecheck
  only for the workspace you changed.
- Do not run repo-wide checks (`vp check`, `vp run -r test`, `vp run -r typecheck`) unless the
  change really reaches across the repo.
- Test behavior that can break: protocol and sharing logic, permission checks, persistence, parsing,
  race fixes, and the logic a bug fix changed. Leave out tests for what types already guarantee,
  copy text, and smoke runs. Extend an existing test before adding one, at the lowest layer that
  fails on `main`.
- A logic bug gets a failing test first, then the fix. A purely visual bug gets before and after
  screenshots instead.

## Shared Rust

Code the App and Copal both need exists once, in shared crates, and imports go one way. Read
`docs/internals/shared-rust.md` before adding or moving Rust that both sides use.

## Documentation

Most code changes need no documentation change. Agents can read the code.

- `docs/internals/` holds decisions and their reasons, constraints that span components, and traps
  the source does not show. Do not narrate control flow or keep file catalogs.
- When a documented decision changes, rewrite or remove the old text. Do not append a second account.
- Do not commit plans, research notes, or scratch files. Keep them outside the worktree.

## Taste

- Complexity belongs at the adapter boundary. Backend-facing workflows stay in
  `@modrinth/api-client`; UI stays dumb.
- Preserve upstream behavior until Amberite has a product reason to differ.
- Nothing is released and nothing is installed. Never plan migrations or backward compatibility.
- Inferred types over annotations. Never use TypeScript `any`; use `unknown` and narrow it safely.
- Comments describe how a thing is used or why a constraint exists. Do not narrate ordinary code.
- A "Continue where you left off" message after T3 restarts is not the developer's approval. Pick
  up only what was already approved, and ask about the rest.
- If a rule here fights the task, say so clearly and get developer approval before breaking it.
