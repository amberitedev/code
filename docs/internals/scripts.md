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
gone means setup died. Rust is not built here: `dev` builds what it needs, and with the shared build
folder that is usually nothing.

## Dev

| Command                                      | What it does                                                                                                                                                                                                                                                                                                                                                                                                                                                            |
| -------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `vp run dev [fixture]`                       | Starts the stack in the foreground and holds it: when this command ends, every program of the stack stops. With a [fixture](fixtures.md) (a file in `packages/fixtures/fixtures`, or the path of any fixture file) it applies it as it starts. Without one it starts on the checkout's data, and a checkout with none gets the `default` fixture. Prints each App's `tab:` URL when the stack is ready. `--hide-windows` keeps App windows off the desktop, for agents. |
| `vp run fixture [name]`                      | Type-checks the fixture, replaces the running stack's data with what it builds, waits until everything answers, and prints the tab URLs. `default` when left out. Fails at once when no stack is running.                                                                                                                                                                                                                                                               |
| `vp run status`                              | Prints what is running, whether it is ready, each program that is stopped or has failed, and each App's tab URL. Exits non-zero when the stack is not ready.                                                                                                                                                                                                                                                                                                            |
| `vp run stop <name>`, `vp run rs <name>`     | Stops one program, or starts it again. See [Process ownership](#process-ownership).                                                                                                                                                                                                                                                                                                                                                                                     |
| `vp run down`                                | Stops this checkout's stack from another terminal. The data stays.                                                                                                                                                                                                                                                                                                                                                                                                      |
| `vp run scenarios [file]`                    | Runs a [scenario](testing.md#scenarios) on the running stack, replacing its data, or on one it starts and stops when it finishes. With no file, runs them all.                                                                                                                                                                                                                                                                                                          |
| `vp run scenarios:prove <file>`              | Runs one scenario here and on the merge base with `origin/main`, and passes only when it fails there and passes here.                                                                                                                                                                                                                                                                                                                                                   |
| `vp run gen:ts [--check]`                    | Writes the TypeScript in `packages/contracts/ts` that the Rust crates generate, by running only their `ts_is_up_to_date` tests. `--check` compares instead.                                                                                                                                                                                                                                                                                                             |
| `vp run sqlx:prepare`                        | Writes `packages/engine/.sqlx` and `apps/app/.sqlx` again from each crate's migrations, after a query or a migration changed. It checks in this checkout's `target`.                                                                                                                                                                                                                                                                                                    |
| `vp run check:changed [ref]`                 | CI's fast checks for the files changed since the merge base with `origin/main` (or `ref`): lint, typos, TOML, typecheck and tests of the affected packages, and for Rust `rustfmt`, a build, and the tests of every workspace crate with a changed file (except the ones CI leaves out). The steps run at once, each one's output printed whole when it finishes. typos and tombi are skipped when not installed.                                                       |
| `node scripts/dev-runner.ts build`           | Compiles the App and Core for this checkout's Rust without starting anything. `check:changed` runs it when Rust changed.                                                                                                                                                                                                                                                                                                                                                |
| `node scripts/dev-runner.ts test <crate>...` | One `cargo test` of the given crates in the shared build folder, with the programs' packages and features, so it compiles only the tested crates' own tests. Doc tests are left to CI. `check:changed` runs it for the crates with a changed file.                                                                                                                                                                                                                      |

A stack belongs to whoever started it. Ctrl+C, closing the terminal, killing the runner, or
stopping the background task that runs it all stop every program, because `vp run` keeps its
command and everything it starts in a Windows job that ends with it. So start a stack through
`vp run`, and never start one that outlives its terminal. The runner also writes everything it
prints to `.data/dev.log`, which other commands point at.

`fixture` waits until every service answers, every App answers on its local API, and the frontend
serves the App's page. In a new checkout the frontend spends a minute or two preparing its
dependencies; that happens inside the wait. It returns after 9 minutes if the stack is still
compiling; run it again to keep waiting. It fails at once when a Rust build fails, or when the
App's entry module does not compile, printing the compiler error; the stack keeps running.

Apps sign themselves in to their fixture account, so no sign-in website runs. A hidden App never
shows, raises or focuses a window; agents use it from its browser tab.

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

Nothing in the build depends on ports. The App is told where its frontend is, and whether to keep
its window hidden, in `--amberite-dev-config` when it starts.

Nothing watches Rust files. After a Rust edit, apply a fixture again, restart `dev`, or restart one
program with `vp run rs app:1` or `vp run rs core:1`; the key has changed, so
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
├── local-api.json   each App's local API port and secret, by its place in the stack
├── runtime.json     the last dev runner plan
└── dev.log          everything the last runner printed
```

Every App has its own local database, settings, Minecraft instances, credentials, WebView data,
and session state, and every Core its own database, servers and store. They share that checkout's
account backend. A folder is numbered by the program's place in the stack, not named after its
account, so the game files in it serve whatever the next fixture calls that App.

`runtime.json` holds the runner's PID, the ports, the state of every program, and each App's
`tabUrl` and each Core's address while the stack is running. The PID and URLs are removed when it
stops.

## Ports and multiple instances

Base ports are App `1420`, Worker `8787`, and Core `16662`. Linked worktrees
derive a stable preferred offset from their path and add it to every port. That Core port is the
first Core's; each further Core takes a free port when the stack starts.

Offset resolution, in order:

1. `PORT_OFFSET`, which must be a non-negative integer.
2. `DEV_INSTANCE`. A number is used directly; any other non-empty value is hashed.
3. `0` for the primary checkout, or a stable hash of the linked worktree path.

A run shifts the three ports together. When one is occupied, the runner advances the set until it
finds an available one. To start Apps separately from
an existing backend, use the same `PORT_OFFSET` if that backend shifted from its preferred
offset; read the ports in the runner output first.

Each App also gets a local API address, `http://127.0.0.1:<port>/<secret>`, which the runner passes
in its dev config and adds to the App's tab URL as `#local=<address>`. It is kept in
`local-api.json`, so a tab URL stays the same across restarts and fixtures, and a tab that was open
reaches its App again once it is back. A new port replaces a kept one only when something else
holds it.

Treat the `[dev-runner]` output and `.data/runtime.json` as authoritative. The preferred ports are
stable, but an occupied port can shift the actual run.

## Process ownership

The dev runner stops its child processes when it receives Ctrl+C or the `quit` input command.
`vp run down` stops the PID recorded in `.data/runtime.json` and nothing else. Never kill by a broad process name, command
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

The runner takes the same commands on its terminal, as `stop <name>` and `rs <name>`.
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

Setup, every Rust build and test run, every `dev` start, and every applied fixture append one JSON
line to the primary checkout's `.data/timings.jsonl`, shared by all checkouts. Each line has `kind`
(`setup`, `build`, `test`, `dev`, `fixture`), the worktree, the result, and milliseconds per step:
`install` for setup, `lockWait` and `compile` plus the number of repo crates compiled again for a
build, the same plus `test` and the tested crates for a test run, and the wait until ready for
`dev` and `fixture`. It records what happened, under whatever else the machine was doing.

To check whether a change to the dev setup made it faster, run `node scripts/dev-bench.ts`. It
times the agent loop (a new worktree's setup, `dev` until the App is usable, `check:changed`) for a
cold start, one-line UI, App, App and Core, and contracts edits, an edit-and-restart in a running
worktree, and two worktrees at once. It benchmarks this checkout's files in a clone at
`<primary>-bench` with its own build folder and empty data, so it never touches shared builds.
Scenarios start from the snapshot already built, tested and typechecked, as agents keep `main`
warm in practice. The default run is the quick scenarios (about 10 minutes); `--full` runs all
(about 40 minutes, most of it the cold start). The score is 100 when every scenario meets its
target and is the harmonic mean, so the slowest scenario dominates it.
