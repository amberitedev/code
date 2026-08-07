# convex

Amberite's durable cloud backend. It stores accounts, profiles, friends, blocks, linked Modrinth accounts, pairing credentials, the minimal Core projection, and sync metadata.

## Important parts

| Area | Purpose |
| --- | --- |
| `schema.ts` | Tables, validators, and indexes |
| `auth.ts`, `_socialRules.ts` | Identity resolution, authorization, invariants, and public DTO helpers |
| `social.ts`, `friends.ts`, `profiles.ts` | The durable social session, relationships, and profiles |
| `presence.ts`, `coreList.ts`, `coreProjection.ts` | Core pairing credentials and the cloud-side Core projection; `presence.ts` is not live presence |
| `sync.ts` | Durable sync profiles, snapshots, and events |
| `bridge.ts`, `realtimeBridge.ts` | Private signed authorization bridge used by the realtime Worker |
| `dev.ts` | Development baselines and helpers enabled by `AMBERITE_DEV_MODE` |

Public DTO changes must be mirrored in `packages/amberite-api`.

## Development deployments

- **Cloud development = `main`:** the shared remote backend should run the Convex code currently in the primary checkout on `main`. Only the primary checkout pushes updates to it. Worktrees can use it when their change does not modify `convex/`.
- **Local development = PR/worktree:** a branch that changes `convex/` runs its own local backend. Starting it selects local Convex for that worktree, and `app:dev` automatically connects apps from that worktree to its local URLs.
- **Production:** separate from this workflow and never selected or deployed by these helpers.

Each worktree has its own `.env.local`, `.convex` database, process state, and automatically assigned ports. Multiple worktrees can run local Convex at the same time; one worktree intentionally runs only one local Convex process.

Local baselines:

- `accounts`: `owner`, `friend`, and `other`, with no relationships.
- `group`: the same accounts plus a group containing `owner` and `friend`.

## Development commands

Run these from the repository root.

| Command | What it does |
| --- | --- |
| `pnpm convex:dev` | Alias for local Convex using the saved baseline or `accounts` |
| `pnpm convex:dev:local -- accounts` | Select and run this worktree's local backend with the accounts baseline; its apps follow automatically |
| `pnpm convex:dev:local -- group` | Select and run this worktree's local backend with the group baseline; its apps follow automatically |
| `pnpm convex:dev:reset -- accounts` | Reset the running local database to accounts |
| `pnpm convex:dev:reset -- group` | Reset the running local database to group |
| `pnpm convex:dev:cloud` | Stop this worktree's local backend and point the worktree back to the shared `main` cloud deployment |
| `pnpm convex:dev:check-cloud` | Warn when the selected cloud development deployment does not match local `main` |
| `pnpm convex:dev:seed-cloud` | From `main`, enable cloud dev mode and ensure `owner`, `friend`, and `other` exist |
| `pnpm convex:dev:status` | Show the selected deployment and local process, branch, URLs, and baseline |
| `pnpm convex:dev:stop` | Stop this worktree's local Convex process |
| `pnpm exec convex dev --configure existing` | One-time cloud development setup for the primary checkout |
| `pnpm exec convex dev --once --tail-logs disable` | From `main`, push its Convex code once after status confirms cloud is selected |

## Non-obvious rules

- Only the primary `main` checkout pushes Convex code to cloud development. A worktree that changes `convex/` uses its local backend and never pushes that branch to the shared cloud deployment.
- Every push to GitHub `main` deploys that exact revision to cloud development and records its commit SHA. `app:dev` checks that marker when cloud development is selected and warns if it differs from local `main`.
- Authorization comes from the authenticated identity and current database state, never client-supplied ownership, roles, audiences, or private fields.
- Do not add transient presence, heartbeats, polling, or client-wide refresh chains. The live-presence protocol belongs in `apps/realtime`.
- Breaking schema changes use expand, migrate/backfill, switch callers, then remove the old shape.
