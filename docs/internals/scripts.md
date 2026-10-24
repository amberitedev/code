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

Node 24.15 or newer is required.

This repository is commonly worked on through [T3 Code](https://github.com/pingdotgg/t3code), a
coding-agent app that gives each task an isolated Git worktree. When T3 Code creates a worktree, it
runs `node scripts/setup-worktree.ts` from `t3.json`. The script installs dependencies (retrying once,
because two worktrees set up at the same time can collide on the shared Git config), copies the
`backend`, `core`, and `scenarios` development data from the primary checkout, and copies the App
library's `.env`. It does not overwrite anything already in the worktree, so it is safe to run
again. `.data/setup.json` records whether it is `running`, `done`, or `failed`.

## Dev

- `vp run dev`: Starts the local account/sharing Worker, the account sign-in website, Core, and the
  App scenarios selected by `dev.json`.
- `vp run dev 1 2 3`: Starts several isolated App installations against those same local services.
- `vp run dev:backend`: Starts the Worker and account website without Apps.
- `vp run dev:app 1 2`: Starts only those App scenarios and their shared frontend. Start
  `dev:backend` first. Core and Minecraft servers are not needed for account/sharing work.
- `vp run dev:core`: Starts only Core. It checks account tokens with the Worker, so start
  `dev:backend` too.
- `node scripts/dev-runner.ts up 1 2`: Starts the same stack in the background, waits until every service answers
  and every App has printed its browser bridge URL, then prints them. It writes to `.data/dev.log`
  and returns after 9 minutes if the stack is still compiling; run it again to keep waiting. It
  fails at once when a Rust build fails. An optional mode comes first:
  `node scripts/dev-runner.ts up dev:backend`. Unlike `vp run dev`, it leaves out the account sign-in
  website and keeps App windows off the desktop, because agents use Apps through the browser
  bridge. `--show-windows` shows them; on a stack already running hidden it fails, so run `down`
  first.
- `node scripts/dev-runner.ts status`: Prints what is running, whether it is ready, and each App's bridge URL. Exits
  non-zero when the stack is not ready.
- `node scripts/dev-runner.ts down`: Stops this checkout's dev runner and everything it started.
- `vp run dev:check`: Prints the processes, state directory, URLs, and ports that a full run would
  use without starting anything.

Scenario numbers passed after the task name override `defaultScenarios` in `dev.json` for that run.

## Rust programs

The runner builds the App and Core with `cargo build` and never runs them from the build output.
Each program is copied to `<target>/dev-bin/<key>/` and run from there, where `<target>` is the
primary checkout's `target` for the App and `apps/core/target` for Core, shared by every worktree.
The key is a hash of everything that goes into the build:

- App: the root `Cargo.toml`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo`, every workspace member
  and path dependency named in `Cargo.toml`, `apps/app-frontend/package.json` (the App's version),
  `packages/app-lib/.env`, the variables named in that file and `.env.prod`, the Cargo arguments, and
  `apps/app/tauri-local.conf.json`.
- Core: `apps/core`, every path dependency named in its `Cargo.toml` (`packages/daedalus`), and the
  root `.cargo` and `Cargo.toml`, which those crates inherit from.

Files are hashed by content as Git would store it, including edits and new files that are not
committed, so the same Rust in two checkouts gives the same key. When the key already has a program,
the runner starts it without Cargo. That is the usual case in a worktree that does not touch Rust.
The six most recently used keys are kept.

Nothing in the build depends on ports. The App is told where its frontend is, and whether to keep
its window hidden, in `--amberite-dev-config` when it starts. `tauri-local.conf.json` turns on the
`dev` capability, which lets a dev App call any Core address.

Nothing watches Rust files. After a Rust edit, restart with `down` then `up`, or `rs <scenario>` and
`rs core` in the foreground runner; the key has changed, so the runner builds first. If a build
fails, the process stays stopped until the next restart.

Builds into one shared output run one at a time. Cargo decides what to recompile from file times
alone, so a crate last compiled from another checkout can look finished here. When the previous
build of a program came from a different checkout, the runner marks the crates that live in the repo
as changed, and Cargo compiles those again. Third-party crates and compiler caches are kept. An older checkout that still runs its App from
`target/debug` holds that file open; the runner renames it aside and builds again.

A fresh dev Core belongs to nobody. Add it once from an App with the code in the `[core]` output;
until then the Servers page asks for a Core. The dev App may call any Core address, which a
packaged build does not allow yet.

The Worker runs through `wrangler dev --local`; D1, Durable Objects, and the R2 bucket that holds
shared files persist in `.data/backend`.
The runner applies local migrations first. It generates a persistent secret in
`.data/backend/dev-secret` and passes it through `.data/backend/.dev.vars`, never command-line
arguments. It also makes a local ticket key in `.data/backend/ticket-key.json`, passes it as
`TICKET_KEY`, and serves its public half from `.data/backend/assets/core/keys.json` in place of the
production keys. No Cloudflare account, billing, remote D1, or remote R2 is used.

Scenario `1` uses username `scenario_1`, email `scenario_1@scenario.invalid`, and password
`Scenario-scenario_1-Local-only!`. Other numbers follow the same pattern. These are real local
database accounts with hashed passwords. The native dev login can seed/sign in these accounts through
a loopback endpoint requiring the runner secret; ordinary email/password login also works. This
shortcut cannot access accounts created through normal signup. Each scenario retains separate App
credentials and settings.
Before starting a scenario, the runner checks that its App is stopped and normalizes copied SQLite
launcher paths to that scenario's directory. Both current and previous launcher directories change
together, so native startup cannot move files from the primary checkout. Copied icon, Java, queued
install, and upload paths follow the new directory; stale copied process records are cleared.

`node apps/backend/tests/accounts.mjs <backend-url>` runs the focused HTTP and native WebSocket
contract proof against the already-running backend. Its private test credentials stay in
`.data/backend-proof/accounts.json`; rerunning after a restart verifies the same accounts persist.

## Dev state

Each checkout owns a gitignored `.data/`:

```text
.data/
├── backend/         persistent local D1, Durable Objects, shared files (R2), and dev secrets
├── core/            shared Core state
├── scenarios/
│   ├── 1/           one complete App installation
│   ├── 2/
│   └── ...
└── runtime.json     the last dev runner plan
```

Every App scenario has its own local database, settings, Minecraft instances, credentials, WebView
data, and session state. Scenarios in one checkout share that checkout's account backend.
Keep backend, Core, and scenarios together when copying the dataset.

Worktree setup copies `backend`, `core`, and `scenarios` from the primary checkout's `.data/` without
overwriting existing state.

`runtime.json` holds the runner's PID, the ports, and each scenario's `bridgeUrl` while the stack is
running. The PID and bridge URLs are removed when it stops.

## Ports and multiple instances

Base ports are App `1420`, account website `3100`, Worker `8787`, and Core `16662`. Linked worktrees
derive a stable preferred offset from their path and add it to every port.

Offset resolution, in order:

1. `AMBERITE_PORT_OFFSET`, which must be a non-negative integer.
2. `AMBERITE_DEV_INSTANCE`. A number is used directly; any other non-empty value is hashed.
3. `0` for the primary checkout, or a stable hash of the linked worktree path.

The runner checks only the ports needed by the selected mode; `up` does not need the account
website's. A full run shifts the App, Worker,
and account website ports together. When a required port is occupied, the runner
advances the complete applicable set until it finds an available one. To start Apps separately from
an existing backend, use the same `AMBERITE_PORT_OFFSET` if that backend shifted from its preferred
offset; read the ports in the runner output first.

Treat the `[dev-runner]` output and `.data/runtime.json` as authoritative. The preferred ports are
stable, but an occupied port can shift the actual run.

## Process ownership

The dev runner stops its child processes when it receives Ctrl+C or the `quit` input command.
A runner started with `node scripts/dev-runner.ts up` has no input; stop it with `node scripts/dev-runner.ts down`, which stops the
PID recorded in `.data/runtime.json` and nothing else. Never kill by a broad process name, command
match, or worktree path: several worktrees may be running Node, Core, and Tauri at the same
time, and a pattern can also match the agent doing the work.

Core and crashed App scenarios restart automatically. Closing an App window normally leaves that
scenario stopped. Enter `rs 1` to start or restart scenario 1, or `rs core` to restart Core; either
rebuilds first when its Rust changed. Three fast failures pause
automatic restarts so a broken launch cannot loop indefinitely; `rs 1` or `rs core` retries manually.

The same commands can be appended as lines to `.data/dev-commands` when the runner has no usable
stdin, which is the case for a runner started detached in the background:

```bash
echo "rs core" >> .data/dev-commands
```

The runner empties that file when it starts and reads new lines every half second.

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
