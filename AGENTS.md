# Amberite

Amberite is a Minecraft launcher for modding and sharing, with self-hosted servers. The desktop App manages the Core, a Rust server manager you run on any machine.

You can think of Amberite as a self-hosted alternative to Modrinth Hosting, BisectHosting and other hosting platforms. The main difference is that Amberite integrates directly into your launcher, like Modrinth Hosting.

Nothing is released, so there's nothing to migrate. But there are some things we never compromise on.

### 1. Easy to use

Amberite is built so anyone can use it, without being any less capable. Unlike a lot of self-hosting platforms, it does not assume you know how things work. We want all users to find Amberite easy, and for the power users to enjoy it even further.

### 2. Free to run

Amberite is open source, but accounts, friends, sharing and more run on a backend we pay for. As Amberite grows, so does the cost of running it. Right now we keep that cost at $0, and we want the backend to stay cheap as our users grow. Weigh the cost impact of every change, and push back on expensive changes that scale poorly.

### 3. Built on Modrinth

Amberite is a fork of the Modrinth monorepo, tracked as `modrinth/main`. We build on Modrinth's content, launcher data and UI. The only part we replace is their launcher. A Modrinth user should be able to switch to Amberite without losing any Modrinth features, and we merge upstream changes regularly. We've rewritten a lot of their code to fit our architecture, but that doesn't mean everything needs rewriting. New changes should fit in with what's already there.

## A note from ilai

I like ambitious ideas, simple systems, and software that feels obvious. Do not preserve complexity just because it already exists. Do not introduce machinery because it looks architecturally impressive. Understand the real constraint, then fight for the smallest model that makes the correct behavior unsurprising.

Channel both "measure twice, cut once" and "yagni". Fight scope creep. Try to honor the dev's intent in both a minimal and realistic fashion.

The rest of this document is meant to help you navigate the codebase and make changes effectively. Think of these instructions less as "hard rules", more as "good defaults". The developer's preferences should be able to override anything here.

Of note: most Amberite work is done by agents in T3 Code worktrees, several at once. Be careful with dev stacks, data, and processes that belong to another worktree or to the developer.

## A small glossary

We need to be on the same page with terminology. When communicating, use this language:

- **you** means the agent reading this file and changing Amberite.
- **we** and **maintainers** mean ilai and the people building Amberite. This is who you are talking to now.
- **user** means a person using Amberite.
- **instance** means a configuration of Minecraft (mods, loader, Minecraft version) in an App or on a Core.
- **server** means a Minecraft server on a Core. Think of it as a hosted world.
- **shared instance** means an instance published through our backend and shared with your friends.
- **share** means one version of a shared instance.
- **stack** means one checkout's dev programs: the backend, its Cores, and its Apps.

## Hit every surface

The most common defect in this repo is a change that works on the path you tested and is missing everywhere else. Before calling work done, walk this list and say which entries applied:

- **Way in, way out.** If you added a way in, add the way out and the way to see it. Share needs unshare, invite needs revoke. A one-way door is an unfinished feature.
- **Contracts.** Anything crossing the wire is an `Endpoint` declared in Rust, and its TypeScript is generated. Change the Rust type and the App, the Cores and the backend all follow.
- **App and Core.** An instance lives on this computer or on a Core. Most instance screens are one shared layout with a provider per side, so an action added on one side is missing on the other.
- **People and states.** A shared instance's owner, its members and a server's players see different things, and a server or a shared instance behaves differently while stopped, running, installing or removed. Check the ones the change touches.
- **Docs.** Check whether the change makes existing guidance inaccurate. Apply the [documentation rules](#documentation) before adding anything.
- **Cost.** If the change touches the backend, say what it does to cost.

## Dev servers

- `vp i` installs. Worktrees get this from the t3.json setup script; if module resolution looks broken, it probably did not run.
- Setup runs in the background. While `.data/setup.json` says `running` and its PID is alive, wait for it and do not run `vp i`. It ends as `done` or `failed` (with the error). After a failure, or with no file, run `vp i` yourself.
- `vp run fixture <name>` starts the programs that fixture names and seeds them.
- Ports derive from the worktree path and are stable across restarts, but read the real ones from `vp run status`, since occupied ports shift.
- Nothing rebuilds Rust for you. After a Rust edit, run `vp run rs <name>`. The first build takes minutes, so run it in the background.
- Stop the stack with `vp run down`. Never kill a process by name or path: other worktrees run stacks too.
- When you finish, run `vp run down`, then `vp run fixture --show-windows` so the developer can try the change.

Every command, the log, and ports: [scripts](docs/internals/scripts.md).

## Verifying

- Before you push, run `vp run check:changed`. It runs CI's fast checks for changed files: lint, typos, TOML, typecheck and tests of the affected packages, and for Rust `rustfmt`, a build, and the tests of changed crates. CI still runs the full suites.
- While you iterate, run only the tests that cover what you changed.
- **Do not run repo-wide checks.** No `vp check`, no `vp run -r test`, no `vp run -r typecheck` unless the developer asks. CI owns the full suite.
- Core and backend behavior changes ship with focused tests for that behavior.
- Test behavior that can break, such as sharing and sync logic, Core and backend permission checks, persistence and migrations, external-format parsing (modpacks, Modrinth's API), race fixes, and the logic a bug fix changed. Leave out tests for what types already guarantee, trivial logic, copy text, smoke runs, mock-call mirrors, and props rendered to static markup.
- Extend before you add. When an existing test or table covers the behavior, add a row or an assertion there. Write a new test only when no case fits.
- Test at one layer: the lowest one that fails on `main`. Do not repeat the case in a wrapper test or at each call site.
- Never wait on sleeps or polling in a test. Await the event that marks the milestone. A test that needs a timeout to pass is wrong, and a flaky test is fixed, not retried.
- Proof for a bug fix depends on the bug. Logic, data, state, and protocol bugs get a failing test first (the fewest that fail for that reason on `main`, or before the fix for a bug this branch introduced), then the fix. Purely visual bugs (spacing, colour, alignment, a layout shift) get before/after screenshots as their proof and no test. A Core- or backend-only fix with no visible surface ships the test plus a short runtime transcript.
- User-visible App changes get one integrated pass in the real App, on a [fixture](docs/internals/fixtures.md) that describes the state the change needs: [test-app](.agents/skills/test-app/SKILL.md). The primary agent does this once after integrating. Subagents do not start their own dev stacks.

## Documentation

Most code changes do not need an internal documentation change. Agents can read the code.

- `docs/internals/` is for architectural decisions and their reasons, constraints that span components, and implementation traps that are hard to discover from the source. Before adding a paragraph, ask what a maintainer would get wrong without it. If reading the relevant code answers the question, leave it out.
- Do not document every feature, enumerate fields or methods, narrate control flow, maintain file catalogs, or append PR summaries. Types, tests, and code already record the implementation.
- A local reason the code can't show goes in a short nearby code comment. Use an internal doc when the reasoning crosses boundaries or needs context the code cannot carry well. Link to the relevant source instead of copying it.
- When a documented decision or constraint changes, rewrite or remove the affected text. Do not append another account of the new behavior. A new internal page needs a distinct, durable reason to exist.

## Plans and work artifacts

- Implementation plans and research notes the developer wants to keep go in `.plans/`, so they can be handed to other agents. Do not read `.plans/` unless the developer points you to a file there. It is not a description of the code.
- Agent scratch files stay outside the worktree.
- A merged PR is the implementation record. Do not preserve a second checklist in the repository.
- Before you end a task, say what is uncommitted or unpushed, and where.

## How it works

The App and a Core do the same things to an instance with one engine and one protocol. Rust imports go one way: `contracts` <- `sharing-client` <- `engine` <- `apps/app`, `apps/core`. The design, and the rules a review checks it against, are in [architecture](docs/internals/architecture.md). Merging and porting upstream: [upstream](docs/internals/upstream.md).

`.repos/modrinth` is a plain checkout of Modrinth's latest `main`, gitignored, for reading and grepping upstream. `SYNCED.json` names its commit. Never edit or import from `.repos/`. If it is missing or stale, run `vp run sync:repos`. Merges and ports use the `modrinth/main` ref, not this copy.

## Taste

- Complexity belongs at the adapter boundary. Backend-facing workflows stay in `@modrinth/api-client`; UI stays dumb.
- Nothing is released and nothing is installed. Never plan migrations or backward compatibility.
- Inferred types over annotations. `any` is the enemy; use `unknown` and narrow it safely.
- Default to no comment. Write one only for a constraint, an external quirk, or an invariant the code can't show, in one line where you can and four at most. Don't restate names or signatures, narrate steps, record history, or explain the fix you just made; the PR carries that.
- No continuously repainting animations (pulse, shimmer, spinners); they peg the GPU on high-refresh displays.
- Security is important, but should not be over-indexed on, especially for dev-only features.
- A "Continue where you left off" message after T3 restarts is not the developer's approval. Pick up only what was already approved, and ask about the rest.
- If a rule here fights the task in front of you, say so loudly and get a human sign-off before breaking it.
