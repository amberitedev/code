# Amberite

Amberite is a Minecraft launcher for modding and sharing, with self-hosted servers. Run the Rust
server (Core) on any machine and manage it from the desktop App.

New code, folders, types and docs say Core. Copal names only the shipped program: its binary,
crate, service, installer and release.

Theseus is Modrinth's codename for the App. It survives in upstream names we keep: the App crate
`theseus_gui`, the Java package `com.modrinth.theseus` and `theseus.jar`, the `modrinth/theseus`
user agent, `THESEUS_CONFIG_DIR` and the `theseus-*.yml` workflows.

You can think of Amberite as Modrinth Hosting, but free and better. Sharing is not tied to a Core
and works for everyone.

This project is very new and has not been deployed or tested anywhere, so never migrate anything.
But there are some things we never compromise on.

### 1. Easy to use

Amberite is built so anyone can use it, without being any less capable. A non-technical user can do
everything they need without setup or jargon. Advanced options exist, but the basics never require
them.

### 2. Performance without compromise

Lots of apps have gotten bogged down with bad tech decisions and "slop". We have not, and we're
proud of Amberite's performance. We regularly audit for performance regressions. Make sure every
change considers its performance impact.

### 3. Free to run

Amberite is free. We never run or host servers: they are self-hosted, and we use those self-hosted
servers to further reduce the cost of our backend. Keep every change cheap, with plenty of headroom.

### 4. Built on Modrinth

Modrinth handles public content, and we use that ecosystem rather than replacing it. We keep the
full Modrinth experience, so users can keep it as their main launcher with Amberite's features added
on top.

Amberite ports Modrinth's social backend from `apps/labrinth` (friends, profiles, sharing, etc.) to
`apps/backend` (Cloudflare Workers and D1) while keeping the same schemas and contracts. Amberite
runs this backend itself, with relevant upstream changes carried over.

## A note from ilai

I like ambitious ideas, simple systems, and software that feels obvious. Do not preserve complexity just because it already exists. Do not introduce machinery because it looks architecturally impressive. Understand the real constraint, then fight for the smallest model that makes the correct behavior unsurprising.

Channel both "measure twice, cut once" and "yagni". Fight scope creep. Try to honor the dev's intent in both a minimal and realistic fashion.

The rest of this document is meant to help you navigate the codebase and make changes effectively. Think of these instructions less as "hard rules", more as "good defaults". The developer's preferences should be able to override anything here.

Of note: most Amberite work is done by agents in T3 Code worktrees, several at once. Be careful
with dev stacks, data, and processes that belong to another worktree or to the developer.

## Hit every surface

The most common defect in this repo is a change that works on the side you tested and is missing on
another. Before calling work done, walk this list and say which entries applied:

- **Way in, way out.** If you added a way in, add the way out and the way to see it. Share needs
  unshare, invite needs revoke. A one-way door is an unfinished feature.
- **One contract.** Every call between the App and a backend (Core, `apps/backend`) is an
  `Endpoint` whose TS is generated into `@amberite/contracts` and called through
  `packages/api-client`. Values Rust owns, such as limits and share rules, are generated the same
  way, never typed twice.
- **Shared UI.** Changes to inherited Modrinth UI or `packages/ui` can affect screens outside the
  one you changed. Check its consumers before treating it as local.
- **Docs.** Check whether the change makes existing guidance inaccurate. Apply the
  [documentation rules](#documentation) before adding anything.

## Dev servers

- `vp i` installs. Worktrees get this from the t3.json setup script; if module resolution looks
  broken, it probably did not run.
- Setup runs in the background. While `.data/setup.json` says `running` and its `pid` is alive,
  wait for it and do not run `vp i`. It ends with `done` or `failed` (with the error). A dead pid,
  `failed`, or no file at all means run `vp run setup` yourself.
- `node scripts/dev-runner.ts up 1 2` starts the backend, one Core, and the Apps for scenarios 1
  and 2 in the background, and returns once each App loads, printing its tab URL. There is no
  `vp run up`: on Windows, vp kills everything its command started when it returns, the background
  stack included. `vp run status` prints the URLs again; `vp run down` stops only this checkout's
  stack. The log is `.data/dev.log`. `vp run dev` is the same stack in the foreground with App
  windows, for a person at a terminal.
- Rust builds once per distinct set of Rust inputs, shared by every checkout. With no Rust changes,
  `up` compiles nothing. Otherwise it compiles only the repo's crates that changed, one build at a
  time across checkouts. `up` gives up waiting after 9 minutes; run it again to keep waiting.
- Nothing watches Rust. After a Rust edit, `vp run down`, then the same `up` command again,
  with its mode and scenario numbers. `up` stops and prints compiler errors, so it is also the
  check for Rust changes.
- Backend-only work needs no App: `node scripts/dev-runner.ts up dev:backend` starts in seconds.
  `node scripts/dev-runner.ts up dev:core` starts only the Core. For the backend and Core together,
  start the full stack.
- Ports derive from the worktree path and are stable across restarts. Read the real ones and the
  tab URLs from `vp run status`, since occupied ports shift.
- Never kill by process name, path, or worktree string. Several worktrees run stacks at once, and
  your own agent's command line holds this worktree's path. Stop what you started with
  `vp run down`, or kill only a PID you captured at spawn.

Ports, builds, and timings: [scripts](docs/internals/scripts.md).

## Test data

An empty database is a bad test. Each worktree gets a copy of the primary checkout's `.data/`
instead of pointing at live state:

- `.data/backend` and `.data/core` are shared by every App in the worktree.
  `.data/scenarios/<number>` is one App installation. Scenarios 1 to 4 are the default set; a new
  number makes another isolated App and account.
- The backend, Core, and scenarios are one dataset. Copy them together.
- Stop the affected process before copying SQLite files, and copy their `-wal` and `-shm` files with
  them.
- Copy in, never symlink. Data flows one way: into your sandbox, never back out.

## Verifying

- Before you push, run `vp run check:changed`. It runs CI's fast checks for changed files: lint,
  typos, TOML, typecheck and tests of the affected packages, and for Rust `rustfmt`, a build, and
  the tests of changed crates. CI still runs the full suites.
- Test behavior that can break, such as sharing and sync logic, Core and backend permission
  checks, persistence and migrations, external-format parsing (modpacks, Modrinth's API), race
  fixes, and the logic a bug fix changed. Leave out tests for what types already guarantee, trivial
  logic, copy text, smoke runs, mock-call mirrors, and props rendered to static markup.
- Extend before you add. When an existing test or table covers the behavior, add a row or an
  assertion there. Write a new test only when no case fits.
- Test at one layer: the lowest one that fails on `main`. Do not repeat the case in a wrapper test
  or at each call site.
- **Do not run repo-wide checks.** No `vp check`, no `vp run -r test`, no `vp run -r typecheck`
  unless the developer asks. CI owns the full suite.
- Proof for a bug fix depends on the bug. Logic, data, state, and protocol bugs get a failing test
  first (the fewest that fail for that reason on `main`, or before the fix for a bug this branch
  introduced), then the fix. Purely visual bugs
  (spacing, colour, alignment, a layout shift) get before/after screenshots as their proof and no
  test. A Core- or backend-only fix with no visible surface ships the test plus a short runtime
  transcript.
- User-visible App changes get one integrated pass in the real App: read
  [test-app](.agents/skills/test-app/SKILL.md). The primary agent does this once after
  integrating. Subagents do not start their own dev stacks.

## Documentation

Most code changes do not need an internal documentation change. Agents can read the code.

- `docs/internals/` is for architectural decisions and their reasons, constraints that span
  components, and implementation traps that are hard to discover from the source. Before adding a
  paragraph, ask what a maintainer would get wrong without it. If reading the relevant code answers
  the question, leave it out.
- Do not document every feature, enumerate fields or methods, narrate control flow, maintain file
  catalogs, or append PR summaries. Types, tests, and code already record the implementation.
- A local reason the code can't show goes in a short nearby code comment. Use an internal doc when
  the reasoning crosses boundaries or needs context the code cannot carry well. Link to the
  relevant source instead of copying it.
- When a documented decision or constraint changes, rewrite or remove the affected text. Do not
  append another account of the new behavior. A new internal page needs a distinct, durable reason
  to exist.

## Plans and work artifacts

- Implementation plans and research notes the developer wants to keep go in `.plans/`, so they can
  be handed to other agents. Do not read `.plans/` unless the developer points you to a file there.
  It is not a description of the code.
- Agent scratch files stay outside the worktree.
- A merged PR is the implementation record. Do not preserve a second checklist in the repository.
- Before you end a task, say what is uncommitted or unpushed, and where.

## Taste

- Complexity belongs at the adapter boundary. Backend-facing workflows stay in
  `@modrinth/api-client`; UI stays dumb.
- Nothing is released and nothing is installed. Never plan migrations or backward compatibility.
- Inferred types over annotations. `any` is the enemy; use `unknown` and narrow it safely.
- Default to no comment. Write one only for a constraint, an external quirk, or an invariant the
  code can't show, in one line where you can and four at most. Don't restate names or signatures,
  narrate steps, record history, or explain the fix you just made; the PR carries that.
- No continuously repainting animations (pulse, shimmer, spinners); they peg the GPU on
  high-refresh displays.
- A "Continue where you left off" message after T3 restarts is not the developer's approval. Pick
  up only what was already approved, and ask about the rest.
- If a rule here fights the task in front of you, say so loudly and get a human sign-off before
  breaking it.

## Additional tips

- Security is important, but should not be over-indexed on, especially for dev-only features.
