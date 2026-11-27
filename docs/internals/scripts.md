# Scripts

> For maintainers and agents.

## First checkout

Amberite uses [Vite+](https://viteplus.dev/guide/). Install the global `vp` command, install
dependencies, then start the development stack:

```bash
curl -fsSL https://vite.plus | bash # Windows: irm https://vite.plus/ps1 | iex
vp i
vp run dev
```

You need:

- Node 24.22.3 or newer.
- Rust through [rustup](https://rustup.rs). `rust-toolchain.toml` pins the version and rustup installs
  it on the first build.
- JDK 17 or newer. The engine's build runs Gradle 9, which uses `JAVA_HOME` when it is set and
  `java` on `PATH` otherwise.
- Tauri's [system packages](https://tauri.app/start/prerequisites/) for your platform.

This repository is commonly worked on through [T3 Code](https://github.com/pingdotgg/t3code), a
coding-agent app that gives each task an isolated Git worktree. When T3 Code creates a worktree, it
runs `node scripts/setup-worktree.ts` from `t3.json`. The script installs dependencies (retrying once,
because two worktrees set up at the same time can collide on the shared Git config) and copies the
engine's `.env`. It copies no data: the first dev start applies the default
[fixture](fixtures.md). It does not overwrite anything already in the worktree, so it is safe to run
again. `.data/setup.json` records
whether it is `running` (with its `pid`), `done`, or `failed` (with an `error`). Any exit,
including Ctrl+C, writes the final state; a hard kill cannot, so a `running` file whose `pid` is
gone means setup died. Rust is not built here: `up` builds what it needs, and with the shared build
folder that is usually nothing.

## Dev

| Command                                      | What it does                                                                                                                                                                                                                                                                                                                                                                                                      |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `vp run fixture <name>`                      | The whole setup: type-checks the [fixture](fixtures.md), starts the stack in the background when none is running, replaces the stack's data with what the fixture builds, waits until everything answers, and prints each App's `tab:` URL. `<name>` is a file in `packages/fixtures/fixtures` or the path of any fixture file, and `default` when left out.                                                      |
| `vp run up`                                  | Starts the stack in the background on the data it already has, with the Apps and Cores of the fixture applied last. A checkout that never had one applied gets the default fixture.                                                                                                                                                                                                                               |
| `vp run dev`                                 | The same stack in the foreground, with the account sign-in website and App windows, for a person at a terminal.                                                                                                                                                                                                                                                                                                   |
| `vp run status`                              | Prints what is running, whether it is ready, each program that is stopped or has failed, and each App's tab URL. Exits non-zero when the stack is not ready.                                                                                                                                                                                                                                                      |
| `vp run stop <name>`, `vp run rs <name>`     | Stops one program, or starts it again. See [Process ownership](#process-ownership).                                                                                                                                                                                                                                                                                                                               |
| `vp run down`                                | Stops this checkout's dev runner and everything it started. The data stays.                                                                                                                                                                                                                                                                                                                                       |
| `vp run scenarios [file]`                    | Runs a [scenario](testing.md#scenarios) on this checkout's stack, starting it when none is running. It replaces the stack's data. With no file, runs them all.                                                                                                                                                                                                                                                    |
| `vp run scenarios:prove <file>`              | Runs one scenario here and on the merge base with `origin/main`, and passes only when it fails there and passes here.                                                                                                                                                                                                                                                                                             |
| `vp run check:changed [ref]`                 | CI's fast checks for the files changed since the merge base with `origin/main` (or `ref`): lint, typos, TOML, typecheck and tests of the affected packages, and for Rust `rustfmt`, a build, and the tests of every workspace crate with a changed file (except the ones CI leaves out). The steps run at once, each one's output printed whole when it finishes. typos and tombi are skipped when not installed. |
| `node scripts/dev-runner.ts build`           | Compiles the App and Core for this checkout's Rust without starting anything. `check:changed` runs it when Rust changed.                                                                                                                                                                                                                                                                                          |
| `node scripts/dev-runner.ts test <crate>...` | One `cargo test` of the given crates in the shared build folder, with the programs' packages and features, so it compiles only the tested crates' own tests. Doc tests are left to CI. `check:changed` runs it for the crates with a changed file.                                                                                                                                                                |

`fixture` and `up` wait until every service answers, every App answers on its local API, and the
frontend serves the App's page. In a new checkout the frontend spends a minute or two preparing its
dependencies; that happens inside the command. They return after 9 minutes if the stack is still
compiling; run the command again to keep waiting. They fail at once when a Rust build fails, or when
the App's entry module does not compile, printing the compiler error; the stack keeps running.
Unlike `vp run dev`, both leave out the account sign-in website and keep App windows off the
desktop, because agents use Apps from browser tabs. `--show-windows` shows them; on a stack
already running hidden it fails, so run `down` first.

A background runner has to outlive the command that started it. On Windows `vp run` puts its
command in a job that kills every process in it when vp exits, and a child process cannot leave
that job. So there the command asks Windows itself to create the runner, which makes it nobody's
child, and leaves its environment in `.data/runner-env.json` for the runner to take, since such a
process starts with a fresh one. The runner writes `.data/dev.log` itself.

`check:changed` builds and tests Rust through `node scripts/dev-runner.ts build` and
`node scripts/dev-runner.ts test <package>...`, which compile in the primary checkout's
`target/dev-shared` like every start does.

## Fixtures

What a fixture is and how to write one: [fixtures](fixtures.md).

## Rust programs

The runner builds the App and Core with one `cargo build` and never runs them from the build output.
Both programs are copied to `<target>/dev-bin/<key>/` and run from there, where `<target>` is the
primary checkout's `target/dev-shared`; every worktree shares those programs. Built apart, Cargo
picks dependency features for each on its own and compiles about 250 crates twice, the engine
among them. The build also passes `--examples`, which builds nothing here but turns on the features
dev-dependencies add, exactly as a test run does, so tests reuse every crate the programs compiled.
The key is a hash of everything that goes into the build:

- The root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo`, the Cargo arguments,
  and the folders of both programs' crates and every local crate they build with, as
  `cargo metadata` reports them. Crates they do not use, such as `toasty`, can change without a
  rebuild.
- `packages/engine/.env` and the variables named in that file and `.env.prod`, which the engine's
  build compiles in.
- `apps/app-frontend/package.json` (the App's version) and `apps/app/tauri-local.conf.json`.

Files are hashed by content as Git would store it, including edits and new files that are not
committed, so the same Rust in two checkouts gives the same key. When the key already has a program,
the runner starts it without Cargo. That is the usual case in a worktree that does not touch Rust.
The six most recently used keys are kept.

When the key has no program here, the runner looks for one CI built before it compiles. The
[Dev programs](../../.github/workflows/dev-programs.yml) workflow builds every Rust change on `main`
for Windows and keeps both programs for 90 days in an artifact named after the key, which `gh`
downloads. So a checkout whose Rust is `main`'s compiles nothing, even on a machine that never built
it. A downloaded program's backtraces name functions but not lines, which keeps the download small.
Two traps:

- The key only matches when `packages/engine/.env` is the copy of `.env.prod` that setup makes, and
  no `.env.local` or environment variable overrides what it sets. Other settings give another key,
  and that checkout compiles its own.
- Only an artifact that workflow made in this repository, on a push or a manual run, for a commit
  that is on `main`, is taken. A pull request can upload one under any name, and the runner runs what it
  downloads.

Nothing in the build depends on ports. The App is told where its frontend is, and whether to keep
its window hidden, in `--amberite-dev-config` when it starts.

Nothing watches Rust files. After a Rust edit, apply a fixture again, restart with `down` then
`up`, or restart one program with `vp run rs app:1` or `vp run rs core:1`; the key has changed, so
the runner builds first. If a build fails, the process stays stopped until the next restart.

Builds and test runs go one at a time across every checkout, at below-normal priority, so several
agents restarting after Rust edits queue up instead of taking the whole machine. A test run holds
its turn until its tests finish, since another build would replace the test programs.

Every checkout compiles into the primary checkout's `target/dev-shared`, like a pnpm store: each
third-party crate is compiled once and kept once, whichever checkout needed it first. Only the
runner compiles there, tests included; a plain `cargo` run, `cargo test` too, uses `target` and
cannot change what the runner trusts. `cargo clean` in the primary checkout
deletes `target/dev-shared` too, so every checkout compiles from scratch next time.

Cargo runs from `<primary>-checkout`, a link (a junction on Windows) that the runner points at the
checkout it builds while it holds the build turn. Compiled crates then hold the same source paths
whichever checkout built them, so the compiler's incremental state carries over between worktrees
(an App edit in a new worktree compiles about as fast as in the last one), and build scripts that
record absolute paths, such as tauri's, do not see a changed crate. Compiler messages name the
real checkout. A crate that reads a file outside its own folder records it by that link too, but
the runner does not date such a file, so keep embedded files inside the crate.

Cargo decides what to recompile from file times alone, so in a shared folder a crate last compiled
from another checkout can look finished here. `target/dev-shared/built-crates.json` records, for
each crate in the repo, the content it was last compiled from: as the programs compile it, which
is also how a test run compiles every crate it does not test, and as each set of tested crates
compiles its own tests. Before a build, a crate that matches gets its files and folders dated 2000,
so Cargo keeps its output; any other crate gets the current time, so Cargo compiles it again.
Inputs outside every crate folder, such as the App's version in `apps/app-frontend/package.json`,
count toward the App's own crate, in its tests too. A crate compiled again, even from the same
content, is newer than the compiled crates that use it, so the next build compiles those again as
well.

An older checkout that still runs its App from `target/dev-shared/debug` holds that file open; the runner
renames it aside and builds again.

The App signs in to a Core with a ticket
on its first request and keeps that session in the OS keychain; set `CORE_PROXY_LOG=1` before
starting an App to log the head of every request its Core proxy sends, with the session token left
out.

The Worker runs through `wrangler dev --local`; D1, Durable Objects, and the R2 bucket that holds
shared version manifests persist in `.data/backend`. Shared files themselves are in a Core's
store, `.data/cores/<n>/store`.
A fixture gives the Worker a database with every migration applied and no rows, made once per
schema by `wrangler d1 migrations apply` and kept in the primary checkout's
`target/dev-shared/fixtures`; a start on existing data applies the migrations to it instead. The
runner generates a persistent secret in
`.data/backend/dev-secret` and passes it through `.data/backend/.dev.vars`, never command-line
arguments. It also makes a local ticket key in `.data/backend/ticket-key.json`, passes it as
`TICKET_KEY`, and serves its public half from `.data/backend/assets/core/keys.json` in place of the
production keys. No Cloudflare account, billing, remote D1, or remote R2 is used.

`node apps/backend/tests/accounts.mjs <backend-url>` runs the focused HTTP and native WebSocket
contract proof against the already-running backend. Its private test credentials stay in
`.data/backend-proof/accounts.json`; rerunning after a restart verifies the same accounts persist.

## Dev state

Each checkout owns a gitignored `.data/`:

```text
.data/
├── backend/         persistent local D1, Durable Objects, manifests (R2), and dev secrets
├── cores/
│   ├── 1/           one complete Core installation
│   └── ...
├── apps/
│   ├── 1/           one complete App installation
│   ├── 2/
│   └── ...
├── stack.json       the Apps and Cores the last applied fixture declared
└── runtime.json     the last dev runner plan
```

Every App has its own local database, settings, Minecraft instances, credentials, WebView data,
and session state, and every Core its own database, servers and store. They share that checkout's
account backend. A folder is numbered by the program's place in the stack, not named after its
account, so the game files in it serve whatever the next fixture calls that App.

`runtime.json` holds the runner's PID, the ports, the state of every program, and each App's
`tabUrl` and each Core's address while the stack is running. The PID and URLs are removed when it
stops.

## Ports and multiple instances

Base ports are App `1420`, account website `3100`, Worker `8787`, and Core `16662`. Linked worktrees
derive a stable preferred offset from their path and add it to every port. That Core port is the
first Core's; each further Core takes a free port when the stack starts.

Offset resolution, in order:

1. `PORT_OFFSET`, which must be a non-negative integer.
2. `DEV_INSTANCE`. A number is used directly; any other non-empty value is hashed.
3. `0` for the primary checkout, or a stable hash of the linked worktree path.

The runner checks only the ports needed by the selected mode; `up` does not need the account
website's. A full run shifts the App, Worker,
and account website ports together. When a required port is occupied, the runner
advances the complete applicable set until it finds an available one. To start Apps separately from
an existing backend, use the same `PORT_OFFSET` if that backend shifted from its preferred
offset; read the ports in the runner output first.

Each App also gets a local API address, `http://127.0.0.1:<port>/<secret>`, which the runner passes
in its dev config and adds to the App's tab URL as `#local=<address>`. The secret is in
`runtime.json` and the `up` and `status` output, never in `.data/dev.log`.

Treat the `[dev-runner]` output and `.data/runtime.json` as authoritative. The preferred ports are
stable, but an occupied port can shift the actual run.

## Process ownership

The dev runner stops its child processes when it receives Ctrl+C or the `quit` input command.
A runner started with `up` has no input; stop it with `vp run down`, which stops the
PID recorded in `.data/runtime.json` and nothing else. Never kill by a broad process name, command
match, or worktree path: several worktrees may be running Node, Core, and Tauri at the same
time, and a pattern can also match the agent doing the work.

Every program follows one rule. An exit nobody asked for is a crash: the runner starts the program
again, waiting twice as long each time, and after five exits in a minute it leaves the program down
and prints why. One program going down never stops the others, the backend included. A stop someone
asked for is not a crash: `vp run stop <name>` keeps a program down until `vp run rs <name>`, which
starts or restarts it and rebuilds first when its Rust changed. Closing an App's window counts as
asking. `<name>` is a label from `vp run status`: `backend`, `core:1`, `app:2`, or an App's account
name. This is how a fixture's state gets a Core that is offline or a backend that is down, without
killing a process by PID.

The foreground runner takes the same commands on its terminal, as `stop <name>` and `rs <name>`.
Both forms write a line to `.data/dev-commands`, which the runner empties when it starts and reads
every half second.

The combined terminal removes repetitive progress output. Vite+ forwards App console
errors and warnings to the same terminal; error and warning notifications are logged there too.

## Check, format, and test

- `vp fmt <files>`: Formats specific files.
- `vp lint <files>`: Lints specific files.
- `vp test run <files>`: Runs specific Vite+ tests.
- `vp run --filter <workspace> typecheck`: Typechecks one workspace.
- `vp check`, `vp run -r test`, and `vp run -r typecheck`: Repository-wide checks owned by CI. Run
  them locally only when the developer asks.
- `vp run build`: Builds the repository. Do not run it unless the developer asks.

## Upstream sync

`node scripts/upstream-sync.ts` merges Modrinth and prints what is left to settle. Its header
lists the other commands.

## Timings

Setup, every Rust build and test run, every `up`, and every applied fixture append one JSON line to
the primary checkout's `.data/timings.jsonl`, shared by all checkouts. Each line has `kind`
(`setup`, `build`, `test`, `up`, `fixture`), the worktree, the result, and milliseconds per step:
`install` for setup, `lookup` (asking for a download), `lockWait` and `compile` plus the number of
repo crates compiled again for a build, or `download` alone when CI's programs were fetched, the
same plus `test` and the tested crates for a test run, and the wait until ready for
`up` and `fixture`. It records what happened, under whatever else the machine was doing.

To check whether a change to the dev setup made it faster, run `node scripts/dev-bench.ts`. It
times the agent loop (a new worktree's setup, `up` until the App is usable, `check:changed`) for a
cold start, one-line UI, App, App and Core, and contracts edits, an edit-and-restart in a running
worktree, and two worktrees at once. It benchmarks this checkout's files in a clone at
`<primary>-bench` with its own build folder and empty data, so it never touches shared builds.
Scenarios start from the snapshot already built, tested and typechecked, as agents keep `main`
warm in practice. The default run is the quick scenarios (about 10 minutes); `--full` runs all
(about 40 minutes, most of it the cold start). The score is 100 when every scenario meets its
target and is the harmonic mean, so the slowest scenario dominates it.
