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

- `vp run dev`: Starts Convex, Core, and the App scenarios selected by `dev.json`.
- `vp run dev 1 2 3`: Starts several isolated App installations against the same Convex and Core.
- `vp run dev:app 1 2`: Starts only those App scenarios. Convex and Core must already be available.
- `vp run dev:core`: Starts only Core.
- `vp run dev:convex`: Starts only Convex.
- `vp run dev:check`: Prints the processes, state directory, URLs, and ports that a full run would
  use without starting anything.

Scenario numbers passed after the task name override `defaultScenarios` in `dev.json` for that run.

## Convex

The primary checkout uses the cloud Convex development deployment selected by `.env.local`.
`vp run dev` runs `convex dev`, which pushes the current Convex functions and watches for further
changes. The runner refuses production and non-development cloud deployments.

Linked worktrees always use the local Convex deployment stored in their `.data/`. The runner selects
that deployment explicitly, even when the worktree has a copied `.env.local` pointing at the cloud.
A worktree cannot push Convex changes to the cloud through the dev runner. This boundary follows the
Git checkout, not the branch name: another branch in the primary checkout still uses cloud Convex,
and a linked worktree named `main` still uses local Convex.

Local Convex runs are prepared with development auth keys, development mode, and the fake accounts
needed by the App scenarios that were launched.

## Dev state

Each checkout owns a gitignored `.data/`:

```text
.data/
├── convex/          local Convex state used by linked worktrees
├── core/            shared Core state
├── scenarios/
│   ├── 1/           one complete App installation
│   ├── 2/
│   └── ...
└── runtime.json     the last dev runner plan
```

Every App scenario has its own local database, settings, Minecraft instances, credentials, WebView
data, and session state. Scenarios in one checkout share that checkout's Core and Convex backend.
Treat the whole `.data/` directory as one dataset; do not create Core-only scenarios.

Worktree setup copies missing entries from the primary checkout's `.data/` without overwriting
existing state. The first dev run also migrates the old root `.convex/` directory into
`.data/convex/` and leaves the compatibility link expected by the Convex CLI.

## Ports and multiple instances

Base ports are App `1420`, local Convex `3210` and `3211`, and Core `16662`. Linked worktrees derive
a stable preferred offset from their path and add it to every port.

Offset resolution, in order:

1. `AMBERITE_PORT_OFFSET`, which must be a non-negative integer.
2. `AMBERITE_DEV_INSTANCE`. A number is used directly; any other non-empty value is hashed.
3. `0` for the primary checkout, or a stable hash of the linked worktree path.

The runner checks only the ports needed by the selected mode. A full local run shifts App, Convex,
and Core together. A full cloud Convex run checks only App and Core. When a required port is occupied,
the runner advances the complete applicable set until it finds an available one.

Treat the `[dev-runner]` output and `.data/runtime.json` as authoritative. The preferred ports are
stable, but an occupied port can shift the actual run.

## Process ownership

The dev runner stops its child processes when it receives Ctrl+C. If you start it in the background,
record its PID when it starts and stop that process only. Never kill by a broad process name, command
match, or worktree path: several worktrees may be running Node, Convex, Core, and Tauri at the same
time, and a pattern can also match the agent doing the work.

## Check, format, and test

- `vp fmt <files>`: Formats specific files.
- `vp lint <files>`: Lints specific files.
- `vp test run <files>`: Runs specific Vite+ tests.
- `vp run --filter <workspace> typecheck`: Typechecks one workspace.
- `vp check`, `vp run -r test`, and `vp run -r typecheck`: Repository-wide checks owned by CI. Run
  them locally only when the developer asks.
- `vp run build`: Builds the repository. Do not run it unless the developer asks.
