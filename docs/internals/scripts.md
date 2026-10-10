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
runs the setup script in `t3.json`. The script installs dependencies, copies missing development data
from the primary checkout, links the App library's environment file, and warms the App dependency
cache. It does not overwrite data already in the worktree.

## Dev

- `vp run dev`: Starts the local account/sharing Worker, the account sign-in website, Core, and the
  App scenarios selected by `dev.json`.
- `vp run dev 1 2 3`: Starts several isolated App installations against those same local services.
- `vp run dev:backend`: Starts the Worker and account website without Apps.
- `vp run dev:app 1 2`: Starts only those App scenarios and their shared frontend. Start
  `dev:backend` first. Core and Minecraft servers are not needed for account/sharing work.
- `vp run dev:core`: Starts only Core. It checks account tokens with the Worker, so start
  `dev:backend` too.
- `vp run dev:check`: Prints the processes, state directory, URLs, and ports that a full run would
  use without starting anything.

Scenario numbers passed after the task name override `defaultScenarios` in `dev.json` for that run.

The Worker runs through `wrangler dev --local`; D1, Durable Objects, and the R2 bucket that holds
shared files persist in `.data/backend`.
The runner applies local migrations first. It generates a persistent secret in
`.data/backend/dev-secret` and passes it through `.data/backend/.dev.vars`, never command-line
arguments. No Cloudflare account, billing, remote D1, or remote R2 is used.

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

Worktree setup copies missing entries from the primary checkout's `.data/` without overwriting
existing state.

## Ports and multiple instances

Base ports are App `1420`, account website `3100`, Worker `8787`, and Core `16662`. Linked worktrees
derive a stable preferred offset from their path and add it to every port.

Offset resolution, in order:

1. `AMBERITE_PORT_OFFSET`, which must be a non-negative integer.
2. `AMBERITE_DEV_INSTANCE`. A number is used directly; any other non-empty value is hashed.
3. `0` for the primary checkout, or a stable hash of the linked worktree path.

The runner checks only the ports needed by the selected mode. A full run shifts the App, Worker,
and account website ports together. When a required port is occupied, the runner
advances the complete applicable set until it finds an available one. To start Apps separately from
an existing backend, use the same `AMBERITE_PORT_OFFSET` if that backend shifted from its preferred
offset; read the ports in the runner output first.

Treat the `[dev-runner]` output and `.data/runtime.json` as authoritative. The preferred ports are
stable, but an occupied port can shift the actual run.

## Process ownership

The dev runner stops its child processes when it receives Ctrl+C or the `quit` input command.
If you start it in the background,
record its PID when it starts and stop that process only. Never kill by a broad process name, command
match, or worktree path: several worktrees may be running Node, Core, and Tauri at the same
time, and a pattern can also match the agent doing the work.

Core and crashed App scenarios restart automatically. Closing an App window normally leaves that
scenario stopped. Enter `rs 1` to start or restart scenario 1, or `rs core` to restart Core. If
Windows still has the App executable open, the runner waits for it to close before relaunching. Three fast failures pause
automatic restarts so a broken launch cannot loop indefinitely; `rs 1` or `rs core` retries manually.

The combined terminal removes repetitive watcher and progress output. Vite+ forwards App console
errors and warnings to the same terminal; error and warning notifications are logged there too.

## Check, format, and test

- `vp fmt <files>`: Formats specific files.
- `vp lint <files>`: Lints specific files.
- `vp test run <files>`: Runs specific Vite+ tests.
- `vp run --filter <workspace> typecheck`: Typechecks one workspace.
- `vp check`, `vp run -r test`, and `vp run -r typecheck`: Repository-wide checks owned by CI. Run
  them locally only when the developer asks.
- `vp run build`: Builds the repository. Do not run it unless the developer asks.
