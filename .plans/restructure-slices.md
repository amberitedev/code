# Restructure slice plan

This is the slice plan for the restructure described in `.plans/restructure.md`. It is temporary
working material and is deleted when the restructure lands.

Decisions are in `restructure-decisions.md` (D-*). Paths after F4 use `packages/engine` (formerly
`packages/app-lib`) and crate `engine` (formerly `theseus`). Every green check also implies
CodeRabbit and CI green on the PR, except where a slice says CodeRabbit skips it. `check:changed`
means `vp run check:changed origin/refactor/restructure-repo` for every slice, F1 included (D-5).
"Bridge tab" and "plain tab" follow D-7: a converted screen is checked in the plain tab, a step that
needs an unconverted command in the bridge tab. Slice ids are stable names, not an order; the
dependency table lists execution order within each lane.

## Amendments

Changes made after the plan was written. They override the slice text below.

- **F1 keeps `packages/blog` and the changelog tooling.** Removing the blog also removes
  `scripts/collect-changelog.ts`, `scripts/build-theseus-release-notes.ts`, the `review-changelog`
  skill and the `changelog:*` scripts, and leaves `theseus-release.yml` calling a deleted script. The
  developer chose the removal without knowing that, so it waits for their confirmation. F1 removes
  only `packages/moderation` and `.idea`, and leaves everything that mentions the blog untouched
  (`_typos.toml`, `packages/ui/package.json`, `ChangelogEntry.vue`, the two scripts, the skill).
- **F3b `fix(upstream): the sync merges renamed folders by path, not by similarity`, after F4.**
  F4's dry runs showed `merge.directoryRenames=true` is not enough: git pairs our moved files by
  similarity, so upstream deletions and hash-named `.sqlx` replacements became rename/delete and
  rename/rename conflicts, and files we had rewritten came back at `packages/app-lib`. F3b rewrites
  the merge base and upstream trees through `renamed` and merges them with `git merge-tree`, so
  every renamed path merges at our name. F7 and the later main syncs that say "as F7" make their
  merge commit with `node scripts/upstream-sync.ts origin/main > summary.json` on a clean checkout,
  not `git -c merge.directoryRenames=true merge`, then reword it to the slice title with
  `git commit --amend` (both parents stay).
- **F2b (after F2, lane C worktree): `check:changed` compiles Rust tests in the shared build
  folder.** F2's test step ran plain `cargo test`, which fills the worktree's own `target/` (22 GB
  after one workspace run) and undid PR #117 for every worktree that touches Rust. The step now
  runs `node scripts/dev-runner.ts test <crate>...`, under the runner's lock, priority and
  `built-crates.json` record; a crate's own tests are trusted only in the checkout that compiled
  them, because they can embed its path.

## Dependency table

| id   | title                                                                                                          | lane       | depends on | est. files             | risk   |
| ---- | -------------------------------------------------------------------------------------------------------------- | ---------- | ---------- | ---------------------- | ------ |
| F1   | chore: the repo reads as Amberite and drops dead upstream packages                                             | foundation | none       | 18                     | low    |
| F2   | ci: every Rust crate's tests run, locally and in CI                                                            | foundation | F1         | 7                      | medium |
| F3   | chore(upstream): the sync knows renamed and ported paths                                                       | foundation | F2         | 7                      | low    |
| F4   | refactor: packages/app-lib becomes packages/engine, crate theseus becomes engine                               | foundation | F3         | 55 (+589 renames)      | medium |
| F5   | feat(contracts): the protocol crate exists, with limits and share rules as its first content                   | foundation | F4         | 45                     | medium |
| F6   | refactor(shared): bedrock's disk and net code lives in sharing-client                                          | foundation | F5         | 20                     | low    |
| F7   | chore: merge main into the integration branch before the lanes fork                                            | foundation | F6         | <30                    | medium |
| A1   | chore(engine): the engine starts without the legacy importer or one-time layout migrations                     | A          | F7         | 10                     | medium |
| A2   | refactor(engine): the engine builds without the global, and two engines run side by side                       | A          | A1         | 10                     | medium |
| A3   | refactor(engine): events go to a sink the host creates                                                         | A          | A2         | 70                     | medium |
| A4   | feat(app): the App serves a local HTTP API, starting with boot info                                            | A          | A3, C2, F5 | 28                     | medium |
| A5a  | feat(app): app events arrive as JSON over one local WebSocket                                                  | A          | A4         | 22                     | high   |
| A5b  | chore(engine): postcard, serde-binhum and the event codec are gone                                             | A          | A5a        | 20 (+generated)        | low    |
| A6   | refactor(engine): instance- and job-keyed statics live on their owners                                         | A          | A5b        | 15                     | low    |
| A7   | refactor(engine): the host sets the Modrinth session and the backend credential                                | A          | A6         | 50                     | medium |
| A8   | refactor(engine): instance, pack and install api functions take the engine                                     | A          | A7         | 120                    | low    |
| A9   | refactor(engine): the remaining api functions take the engine                                                  | A          | A8         | 70                     | low    |
| A10  | refactor(engine): the global is gone and State is Engine                                                       | A          | A9         | 200                    | low    |
| A10b | feat(app): the engine starts in App setup and initialize_state waits on the socket                             | A          | A10        | 12                     | medium |
| A11  | feat(app): local images load over HTTP instead of asset://                                                     | A          | A10b       | 20                     | medium |
| A12  | feat(app): dialogs, links, folders and shortcuts are App routes, and window chrome works in a tab              | A          | A11        | 28                     | medium |
| A13  | feat(engine): engine routes behind Access serve the Library's instance reads                                   | A          | A12        | 27                     | medium |
| A14  | feat(app): settings, the opening command, onboarding and hidden instances come over HTTP                       | A          | A13        | 20                     | medium |
| A16  | feat(engine): tags, metadata and the Modrinth cache come over HTTP                                             | A          | A14        | 12                     | low    |
| A17  | feat(engine): logs and processes come over HTTP                                                                | A          | A16        | 10                     | low    |
| A18  | feat(app): Java and launcher import come over HTTP                                                             | A          | A17        | 12                     | low    |
| A19  | refactor(app-frontend): user, preference and report helpers call Modrinth's client directly                    | A          | A18        | 10                     | low    |
| A30  | feat(app): game options, synced packs and screenshots come over HTTP                                           | A          | A19        | 16                     | medium |
| A15  | feat(app): Minecraft and Modrinth sign-in run from App routes; the shell works in a plain tab                  | A          | A30        | 12                     | medium |
| A20  | feat(app): friends and skins come over HTTP                                                                    | A          | A15        | 14                     | medium |
| A21  | feat(app): long file operations start, report progress as events and cancel by op id                           | A          | A20        | 16                     | medium |
| A22  | feat(app): updates are checked, downloaded and installed through App routes                                    | A          | A21        | 9                      | medium |
| A23  | refactor(app): the Core proxy lives in apps/app/src/cores                                                      | A          | A22, B1    | 7 (5 renames)          | low    |
| A24  | feat(app): Cores are reached through one App proxy listener                                                    | A          | A23        | 12                     | high   |
| A25  | refactor(app): the App resolves the data directory and finishes directory moves before the engine starts       | A          | A24        | 15                     | medium |
| A26  | refactor(app): launches take credentials, launch defaults and game options from the App                        | A          | A25        | 12                     | medium |
| A27  | feat(app): worlds and server joins come over HTTP                                                              | A          | A26        | 12                     | medium |
| A28  | refactor(app): Discord, friends status and the window follow process events                                    | A          | A27        | 10                     | medium |
| A29  | refactor(app): synced options, packs, screenshots, locales and onboarding follow instance events               | A          | A28, B10   | 30                     | high   |
| A31  | feat(engine): the files tab and content storage read and write raw bytes over HTTP                             | A          | A29        | 12                     | medium |
| A32  | feat(engine): installs come over HTTP                                                                          | A          | A31        | 12                     | medium |
| A33  | feat(engine): instance content and lifecycle come over HTTP, and play goes through the App                     | A          | A32        | 20                     | high   |
| A34  | feat(engine): sharing comes over HTTP and the last engine command is gone                                      | A          | A33, B10   | 22                     | high   |
| A40  | refactor(app-frontend): our Core and notification screens call contracts directly                              | A          | A34        | 22                     | medium |
| A35  | refactor(app): accounts, skins, friends and Discord code moves into apps/app                                   | A          | A40        | 40 (moves)             | low    |
| A36  | refactor(app): synced options, screenshots, library, onboarding, settings and desktop code moves into apps/app | A          | A35        | 50 (moves)             | low    |
| A37  | refactor(app): desktop domains call the backend through contracts                                              | A          | A36, B8    | 15                     | medium |
| A38  | refactor(engine): engine.db and app.db are separate fresh schemas                                              | A          | A37        | 42 (+generated)        | high   |
| A39  | build(engine): the engine links no desktop crate, and CI keeps it that way                                     | A          | A38        | 4                      | low    |
| B1   | fix(protocol): one ApiError for the backend, Core, the store and their clients                                 | B          | F7         | 55                     | medium |
| B2   | refactor(backend): one fresh D1 schema with contract table names                                               | B          | B1         | 30                     | low    |
| B3   | refactor(sharing-client): no ProjectType, ModLoader or bedrock content                                         | B          | B2         | 10                     | low    |
| B4   | refactor(backend): Core-facing backend routes are contract Endpoints                                           | B          | B3         | 32                     | medium |
| B5   | refactor(core): Core's state struct is CoreState                                                               | B          | B4         | 40                     | low    |
| B6   | feat(core): Core-native routes are contract Endpoints and /health reports the protocol                         | B          | B5         | 60                     | medium |
| B7   | refactor(social): labrinth-shaped social routes and the friends socket are contract Endpoints                  | B          | B6         | 18                     | medium |
| B8   | refactor(accounts): labrinth-shaped account and dev routes are contract Endpoints                              | B          | B7         | 14                     | medium |
| B9   | refactor(store): the store's five routes are contract Endpoints                                                | B          | B8, A10    | 18                     | medium |
| B10  | refactor(sharing): shared instance routes are contract Endpoints and upstream's module is upstream again       | B          | B9         | 90                     | high   |
| C1   | refactor(app-frontend): native calls outside helpers go through one platform seam                              | C          | F7         | 36                     | low    |
| C2   | feat(dev): the dev runner gives each App a fixed local address and prints a plain-tab URL                      | C          | C1         | 8                      | low    |
| C3   | chore: merge main into the integration branch before the Core lane                                             | C          | A39, B10   | <30                    | medium |
| C4   | feat(dev): agents use the App in a plain browser tab and the browser bridge is gone                            | C          | C3         | 20 (+14 deleted)       | medium |
| D1   | feat(core): the Core starts its own engine and checks access through CoreAccess                                | D          | C3         | 45                     | medium |
| D2   | feat(engine): hosts can choose an instance's id and folder                                                     | D          | D1         | 6                      | low    |
| D3   | refactor(engine): Core's sandbox, archive checks, pack-host guard and server jar inspection live in the engine | D          | D2         | 30                     | medium |
| D4   | feat(engine): a server-side engine lists, installs and exports server content                                  | D          | D3         | 42                     | high   |
| D5   | feat(core): a Core's content and sharing run on the engine, and the duplicate half is gone                     | D          | D4         | 145 (mostly deletions) | high   |
| D6   | feat(engine): the engine installs servers and builds their Java command                                        | D          | D5         | 35                     | high   |
| D7   | feat(core): files, icons and download links run on the engine                                                  | D          | D6         | 40                     | high   |
| D8   | feat(core): one event socket with topics replaces the console socket                                           | D          | D7         | 20                     | high   |
| D9   | feat(core): Core server routes drop their Archon shapes                                                        | D          | D8         | 45                     | medium |
| D10  | chore(engine): bedrock is gone                                                                                 | D          | D9, C4     | 15 (+64 deleted)       | low    |
| Z1   | chore: merge main into the integration branch for the final pass                                               | final      | D10, C4    | <30                    | medium |
| Z2   | docs: real docs replace the restructure plan                                                                   | final      | Z1         | 27                     | low    |
| Z3   | test: integrated pass, rebuilt dataset and the final pull request                                              | final      | Z2         | 0                      | medium |

## Lanes

### Foundation (serial, before the fork)

F1 to F7 land one at a time on `refactor/restructure-repo`. After F7 every lane forks from fresh
`origin/refactor/restructure-repo` in its own T3 Code worktree with its own `target/` (D-109).
Inside a lane, slices are serial in the table's order (one Rust implementer per target folder).

### Lane ownership

A file has one owner at a time. Ownership moves only at the merge points below. Lockfiles and
generated files (`Cargo.lock`, `pnpm-lock.yaml`, `.sqlx`, `packages/contracts/ts/*.ts` except the
hand-written `index.ts`) are regenerated on rebase, not owned. `packages/ui/**` is owned by B, then
D, and no slice edits it (developer decision 5).

| Lane                                     | Owns during the lane phase (F7 to MP3)                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                        |
| ---------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| A engine and App host                    | `packages/engine/**` except the sharing paths during the B window (below); `apps/app/**` except `apps/app/src/api/cores/**` until MPc; `apps/app-frontend/**` from MPa, except `apps/app-frontend/vite.config.ts` and the B files below; `packages/api-client/src/platform/**`; `packages/serde-binhum/**`; `scripts/scenario-state.ts` and its test; root `Cargo.toml`; `.cargo/config.toml`; `.gitattributes`; `.gitignore`; `upstream.json`; `.github/workflows/ci.yml`; AGENTS.md's "Test data" section (A38 only)                                                                                                                                                                                                                                                                                                                        |
| B protocol, stores, backend, Core-native | `packages/contracts/**` (B owns `error.rs` until MP2); `packages/ariadne/**`; `packages/sharing-client/**`; `packages/sharing-storage/**`; `packages/bedrock/**`; `apps/backend/**`; `apps/core/**`; `packages/api-client/src/**` except `platform/**`; `packages/ui/**` (no edits); `.prettierignore`; `docs/internals/shared-rust.md`; `scripts/self-hosted-routing.test.ts`; `apps/app/src/api/cores/**` until MPc; in `apps/app-frontend/src/`: `features/servers/linked-servers.ts`, `features/settings/sharing/**`, `features/settings/account/cores-settings.vue`, `composables/use-shared-instance-event.ts`, and `pages/instance/layout.vue` until B6 merges; in the B window also `packages/engine/src/api/instance/shared/**`, `packages/engine/src/self_hosted/pool/**`, `packages/engine/src/install/{shared_instance,model}.rs` |
| C frontend platform and tooling          | `apps/app-frontend/src/**` until MPa; `apps/app-frontend/vite.config.ts`; root `vite.config.ts`; `scripts/**` except `scenario-state.ts` and `self-hosted-routing.test.ts`; `.github/**` except `ci.yml`; `packages/browser-bridge/**`; `turbo.jsonc`; root `package.json`; `.coderabbit.yaml`; `README.md`; `AGENTS.md` except the "Test data" section; `docs/**` except `shared-rust.md`; `.agents/**`; `.claude/**`                                                                                                                                                                                                                                                                                                                                                                                                                        |

After MP3 (lane D and C4 in parallel):

| Lane       | Owns                                                                                                                                                                                                                                                                                                                                                            |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| C (C3, C4) | `apps/app/**`, `apps/app-frontend/**` except `src/platform/adapters/file-download.ts`, `packages/browser-bridge/**`, `scripts/**`, `.github/**`, docs, skills, root `Cargo.toml` until C4 merges                                                                                                                                                                |
| D          | `apps/core/**`, `packages/engine/**`, `packages/contracts/**`, `packages/api-client/src/**` (including `platform/**`), `packages/ui/**` (no edits), `packages/bedrock/**`, `packages/sharing-*/**`, `apps/app-frontend/src/platform/adapters/file-download.ts`, `upstream.json`, `.prettierignore`, `docs/internals/shared-rust.md`, root `Cargo.toml` after C4 |

### Merge points

- **MP0** F7 merged: lanes A, B, C fork.
- **MPa** C1 merged: `apps/app-frontend/src/**` passes from C to A (except B's files). C2 may start.
- **MPb** C2 merged: the runner passes `localPort`/`localSecret` and prints `#local=` URLs. A4 may
  start, so no `vp run up` URL is ever broken between A4 and the runner change.
- **MPc** B1 merged: `apps/app/src/api/cores/**` passes from B to A. A23 may start.
- **MP1** A10 merged: B9 and B10 may start. From MP1 until B10 merges ("the B window"), A edits none
  of `packages/engine/src/api/instance/shared/**`, `packages/engine/src/self_hosted/pool/**`,
  `packages/engine/src/install/{shared_instance,model}.rs`. A10b to A28 and A30 do not need them.
- **MP2** B10 merged: lane B ends. The engine sharing paths, `packages/contracts/**`,
  `packages/ariadne/**` and the rest of B's files pass to A until MP3. A29, A34 and A40 may start
  (A37 also needs B8).
- **MP3** A39 and B10 merged, then C3 (main merge) merged: lane D and C4 start.
- **MP4** D10 and C4 merged: Z1 starts.

At most three lanes run at once: A, B, C between MP0 and MP3; C and D after MP3. Lane C is idle from
C2 to C3; nothing it could take avoids A's files.

### Data resets

Per D-109, a slice worktree based on a schema slice deletes what that slice made unusable before its
first `vp run up`, then rebuilds what its check needs with scripts.md's "Regenerating the sharing
dataset" steps: after B2, `.data/backend`; after A38, `.data/scenarios`; after D4,
`.data/core/engine` and `.data/scenarios`; after D5, `.data/core`. `vp run down` first; never touch
another worktree's or the primary checkout's `.data`.

## What goes away, and which slice deletes it

| Item                                                                                           | Slice                                                    |
| ---------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| `packages/bedrock`: `limits`                                                                   | F5                                                       |
| `packages/bedrock`: `disk::free`, `CoreAddress`, `net/**`                                      | F6                                                       |
| `packages/bedrock`: jar `server.rs` (moved to the engine; Core imports the engine's copy)      | D3                                                       |
| `packages/bedrock`: `content/`, jar clone, store clone, glue                                   | D5                                                       |
| `packages/bedrock` crate, `ports.toml`, borrowed files back in the engine                      | D10                                                      |
| Duplicated content, files and sharing half of `apps/core/src/state/servers`                    | D5, D7                                                   |
| Core's Archon-shaped content and share routes                                                  | D5                                                       |
| Core's Archon-shaped file and log routes                                                       | D7                                                       |
| Remaining Archon/Kyros-shaped Core routes                                                      | D9                                                       |
| api-client's `copal` module                                                                    | B6 (client modules); D5 (last generated types)           |
| api-client's `archon/cores/v1.ts`, the `Cores` types and `labrinth/notifications/v3.ts` (ours) | A40                                                      |
| Amberite edits to upstream's `shared-instances` module                                         | B10 (callers of our additions: A40)                      |
| Second copy of sharing (Core share commands)                                                   | D5                                                       |
| sharing-client's hand-written backend functions                                                | B10                                                      |
| `pool/store.rs` hand client                                                                    | B9                                                       |
| Core `access/sync.rs` and `pairing.rs` raw clients, store `keys.rs` raw reqwest                | B4                                                       |
| Backend hand-written TS types                                                                  | B4, B7 (with the friends socket's), B8, B10              |
| `packages/browser-bridge`                                                                      | C4                                                       |
| App's Tauri commands for engine and desktop features                                           | A4 to A34, module by module (last engine command in A34) |
| The `utils` plugin                                                                             | A21 (last of its commands)                               |
| The Rust `initialize_state` command                                                            | A10b                                                     |
| Tauri `Channel` for events                                                                     | A5a                                                      |
| "Copal never depends on app-lib" rule and `shared-rust.md`                                     | D10 (file); Z2 (real docs)                               |

Developer decisions: 1 (F1, B6, D9 keep `copal`), 2 (F4), 3 (A38, B2, D5), 4 (F1; DO-NOT-READ docs
reported in Z3), 5 (D5, D7, D8, D9 adapter; B10 and B7 leave `packages/ui` alone; A40 moves our own
screens), 6 (F1), 7 (Z3; dead commands go only with their deleted plugin, D-37).

## Slices

### F1 chore: the repo reads as Amberite and drops dead upstream packages

- Lane: foundation
- Depends on: none
- Goal: CodeRabbit reviews slices against the doc's Rules; the README describes Amberite; AGENTS.md
  states the Core/Copal rule; `packages/moderation`, `packages/blog`, `.idea` and the blog-only
  changelog tooling are gone and stay gone on sync.
- Scope: `.coderabbit.yaml`, `README.md`, `AGENTS.md`, `upstream.json`, `crowdin.yml`,
  `turbo.jsonc`, `package.json`, `.vscode/settings.json`, `_typos.toml`, `packages/ui/package.json`,
  `packages/ui/src/components/changelog/ChangelogEntry.vue`,
  `scripts/{collect-changelog,build-theseus-release-notes}.ts`,
  `.agents/skills/review-changelog/**`, `packages/{blog,moderation}/**`, `.idea/**`,
  `pnpm-lock.yaml`. About 18 counted files (about 630 deletions are path-filtered).
- Adds / Changes / Deletes:
  - Adds `.coderabbit.yaml` (decisions D-108; path filter includes `packages/contracts/ts/**`), a
    short Amberite `README.md` (what it is, `vp i` and `vp run dev`, AGENTS.md and `docs/internals`,
    credit and link to Modrinth, license pointer to `COPYING.md`), and the AGENTS.md Core/Copal line
    (D-9).
  - Changes `upstream.json` ignore (+ `packages/blog`, `packages/moderation`, `.idea`, the two
    scripts, `.agents/skills/review-changelog`); removes moderation from `crowdin.yml:15-18`,
    `turbo.jsonc:88-89`, `package.json` `prepr:frontend:lib`, `.vscode/settings.json:38`; drops blog
    from `_typos.toml:8`, the `changelog:*` scripts and `packages/ui/package.json:46`; inlines the
    4-field `VersionEntry` type in `ChangelogEntry.vue:45`.
  - Deletes `packages/blog`, `packages/moderation`, `.idea`, the two scripts and the skill.
- Green check: `vp i`; `check:changed`; `vp run --filter @modrinth/app-frontend typecheck`;
  `git grep -n "@modrinth/blog\|@modrinth/moderation\|packages/blog\|packages/moderation" -- ':!upstream.json' ':!.github/workflows/upstream-sync.yml' ':!.claude'`
  prints nothing; `vp run up 1`, open the bridge URL, Library and Settings load. CodeRabbit posts a
  review using the new config.
- Risk: low. Deletions of unused code and one inlined type.
- Traps: `.github/workflows/upstream-sync.yml:106` diffs `packages/blog/changelog.ts` between
  upstream refs; leave it and the skill's `changelog.diff`. `scripts/coverage-i18n.ts:72-73,770`
  lists `packages/moderation` next to the already-missing `apps/frontend`; leave it (upstream).
  `theseus-release.yml:64` calls the deleted release-notes script: report, do not edit. Never put
  `README.md` in ignore (`scripts/upstream-sync.ts:53-55` deletes ignored paths).

### F2 ci: every Rust crate's tests run, locally and in CI

- Lane: foundation
- Depends on: F1
- Goal: CI runs `cargo test --workspace --exclude toasty --locked` in one Rust job with a broad
  filter and a desktop-crate guard; `check:changed` runs the tests of touched crates; copal-release
  and the dev runner can build a Core that depends on the engine.
- Scope: `.github/workflows/{ci,copal-release}.yml`, `scripts/check-changed.ts`,
  `scripts/dev-runner.ts`, `scripts/dev-runner.test.ts`, `docs/internals/scripts.md`, `AGENTS.md`.
  About 7 files.
- Adds / Changes / Deletes:
  - `ci.yml`: deletes jobs `core` and `app` and the hand-listed `core` filter (lines 47-69,
    105-157); adds the `rust` job and filter (`**/*.rs`, `**/Cargo.toml`, `Cargo.lock`,
    `rust-toolchain.toml`, `rustfmt.toml`, `.cargo/**`, `**/migrations/**`, `**/.sqlx/**`,
    `packages/*/java/**`, `packages/*/build.rs`, `packages/*/.env*`, `packages/contracts/**`,
    `apps/app/**` (tauri-build checks capabilities and `tauri*.conf.json` at compile time),
    `apps/core/scripts/**`, `.github/workflows/ci.yml`) with checkout, Tauri apt deps, pnpm install,
    toolchain, `.env` copy, `cargo fmt --check`, `cargo shear`,
    `cargo check -p theseus_gui --features browser-bridge --locked`, the workspace test,
    `sh -n apps/core/scripts/install.sh`, and the guard
    `! cargo tree --locked -e normal -p copal -i tauri`.
  - `copal-release.yml`: `actions/setup-java` Temurin 17 and
    `cp packages/app-lib/.env.prod packages/app-lib/.env` before `cargo build -p copal` (line 63).
  - `dev-runner.ts`: the Core build spec (line 677) gets the same compiled env and `.env` inputs as
    the App spec (lines 562-570, 745-760) through one helper.
  - `check-changed.ts`: a Rust test step running `cargo test --locked -p <crate>` with
    `SQLX_OFFLINE=true` for each workspace crate (from `cargo metadata --no-deps`) containing a
    touched file, toasty excluded.
  - scripts.md (check:changed, Core build key inputs) and the AGENTS.md Verifying line.
- Green check: `check:changed`; `vp test run scripts/dev-runner.test.ts`;
  `SQLX_OFFLINE=true cargo test --workspace --exclude toasty --locked` passes on Windows; CI `rust`
  job green; `vp run up dev:core` starts the Core.
- Risk: medium. Tests of sharing-client (47), sharing-storage (29), bedrock (34) and app-lib (38)
  have never run in CI.
- Traps: bedrock's `typescript_is_up_to_date` (`packages/bedrock/src/limits.rs:63`) and
  sharing-client's rules check (`packages/sharing-client/src/rules/mod.rs:217`) may be stale;
  regenerate the TS. If an upstream crate's test needs the network or fails only on Linux, `--skip`
  it in ci.yml with a one-line reason; never edit the upstream test. The old Core job used
  `cache-workspaces: apps/core` (`ci.yml:121`); the new job caches the root. The old `app` filter
  missed `packages/*/build.rs` and migrations. Changing the Core build key rebuilds copal once in
  `target/dev-shared`. app-lib's `build.rs` runs Gradle and reads `.env` (compile-time `env!`): the
  Core spec and copal-release need both.

### F3 chore(upstream): the sync knows renamed and ported paths

- Lane: foundation
- Depends on: F2
- Goal: `upstream-sync.ts` merges with `merge.directoryRenames=true`, maps upstream paths through
  `renamed`, never resurrects `ported` files and reports their commits for porting; the rules live
  in `docs/internals/upstream.md`.
- Scope: `scripts/upstream-sync.ts`, `scripts/upstream-paths.ts` (new),
  `scripts/upstream-paths.test.ts` (new), `upstream.json`, `docs/internals/upstream.md` (new),
  `docs/internals/scripts.md`, `.claude/skills/upstream-sync/SKILL.md`. About 7 files.
- Adds / Changes / Deletes:
  - Adds `scripts/upstream-paths.ts` (`ourPath`, `bucketOf` over parsed `--name-status` changes) and
    its test rows: renamed prefix maps, `ported` beats `ignored`, an ignore key is deleted by its
    mapped name, `overlap` matches a renamed path.
  - `upstream-sync.ts:48` becomes `git -c merge.directoryRenames=true merge ...`; deletes `ignore`
    and `ported` keys by mapped name (line 53); bucket `watch` becomes `port`.
  - `upstream.json`: `watch` becomes `ported` (labrinth paths to `apps/backend/src/...`), adds empty
    `renamed`.
  - Adds `docs/internals/upstream.md`: the workflow and token paragraph moved from
    `scripts.md:248-261`, the three config keys, and the doc's porting rules (an `api/` function or
    `#[tauri::command]` becomes an Endpoint plus engine handler and the helper body calls
    `client.local`; a new event becomes an `Event` variant; a plugin call becomes an App route; a
    desktop domain change goes to its `apps/app/src/<domain>` folder and App migrations; a new
    upstream migration is ported into the fresh engine or App schema; Archon calls in our screens go
    to the engine API; upstream's archon and shared-instances modules stay for upstream pages).
    scripts.md keeps one link.
  - The skill: bucket names, step 7 "Port" covers every `port` commit, rules link upstream.md.
- Green check: `vp test run scripts/upstream-paths.test.ts`; `check:changed`; in a scratch worktree
  outside the repo on a throwaway branch, `node scripts/upstream-sync.ts > summary.json` shows the
  labrinth commits under `port`; remove the scratch worktree and branch.
- Risk: low. Dev-only script with a unit test.
- Traps: deletion runs against our paths (`upstream-sync.ts:53`), bucketing against upstream paths
  (`:108-111`), and `ours` (`:44`) holds our paths: map before every comparison. `git rm` of an
  unmerged (modify/delete) `ported` path must happen before the conflict list (`:65`), or line 66's
  `git add` resurrects it. Rename-detected moves leave no upstream path in the index: skip the
  `git rm` and still bucket the commit as `port`.

### F4 refactor: packages/app-lib becomes packages/engine, crate theseus becomes engine

- Lane: foundation
- Depends on: F3
- Goal: the library lives at `packages/engine` as crate `engine`, every path, log filter and tool
  follows, and a dry-run sync proves upstream app-lib changes land in the renamed folder.
- Scope: `packages/app-lib/** -> packages/engine/**` (589 renames), plus about 55 edited files: root
  `Cargo.toml` (6, 131), `packages/engine/Cargo.toml:2-3`, `apps/app/Cargo.toml:41,96`, the 27
  `apps/app/src` files with `theseus::` (319 lines), `packages/engine/src/logger.rs:5-14,27,117`,
  `apps/app/src/main.rs:149-156`, `packages/bedrock/src/state/mod.rs:4`,
  `packages/bedrock/src/state/instances/model/mod.rs:4-24`, `packages/bedrock/src/util/mod.rs`,
  `turbo.jsonc:12,189,200`, `apps/app/turbo.jsonc:8,21-28,48-55`, `.prettierignore:11,17`,
  `.gitattributes:37-40`, `ci.yml` (.env copy), `copal-release.yml` (.env copy),
  `theseus-build.yml:11,149,152`, `scripts/dev-runner.ts:565,566,737,747`,
  `scripts/setup-worktree.ts:3,15`, `apps/app-frontend/vite.config.ts:10,15-30`,
  `apps/app-frontend/src/generated/app-events/README.md:6`, `upstream.json`,
  `docs/internals/{scripts,shared-rust}.md`, `AGENTS.md`, tracked `.env.*` comments,
  `apps/backend/tests/sharing.contract.test.mjs:7`, `apps/core/src/state/{mod,servers/mod}.rs:1`.
  `Cargo.lock` regenerated.
- Adds / Changes / Deletes:
  - `git mv packages/app-lib packages/engine` (tracked files, including `.env.staging`, `.env.prod`,
    `.env.prod-with-staging-archon`, `.taurignore`, `.cargo/config.toml`, `prettier.config.cjs`,
    `java/`, `.sqlx/`, `migrations/`).
  - Crate `name = "engine"`; workspace dep `engine = { path = "packages/engine" }`; `theseus::`
    becomes `engine::` in apps/app; `export-app-events = ["engine/export-ts"]`.
  - Log filters `theseus=` become `engine=` in `logger.rs` and `dev-runner.ts:737`.
  - bedrock's `#[path = ".../app-lib/src/..."]` strings become `.../engine/src/...`.
  - `upstream.json` `renamed: { "packages/app-lib": "packages/engine" }`; the ignore key
    `packages/app-lib/AGENTS.md` stays (upstream namespace).
  - `setup-worktree.ts` copies `packages/engine/.env` from primary, falling back to
    `packages/app-lib/.env`.
  - AGENTS.md gains the theseus line (D-10).
- Green check: `check:changed`; `SQLX_OFFLINE=true cargo test -p engine -p bedrock --locked`;
  `git grep -n "packages/app-lib" -- ':!.plans' ':!upstream.json' ':!packages/bedrock/ports.toml'`
  lists only upstream files; `git grep -nw theseus -- '*.rs' '*.toml' '*.ts' '*.jsonc'` lists only
  `theseus_gui`, Java names and the `modrinth/theseus` UA; `vp run up 1`, open the bridge URL, open
  an instance's content tab, App logs still appear in `.data/dev.log`. Dry runs in scratch worktrees
  outside the repo, removed after: (1) on a throwaway branch,
  `node scripts/upstream-sync.ts > summary.json`: `git ls-files packages/app-lib` is empty and the
  upstream app-lib commit is in `overlap`; (2) replay: branch at an upstream commit at least 80
  app-lib commits before `fcd4fe1fa`, apply the same `git mv` and crate rename as one commit, then
  `git -c merge.directoryRenames=true merge --no-commit fcd4fe1fa`: no conflicts under
  `packages/engine` other than lines naming the crate, nothing recreated under `packages/app-lib`.
  Record both conflict counts in the PR; if D-10's fallback triggers, revert only the crate name.
- Risk: medium. Mechanical but wide; every checkout's next build recompiles the engine and App
  (about 11 min cold). The PR is over CodeRabbit's 300-file limit and `packages/engine/**` cannot be
  path-filtered, so CodeRabbit is expected to skip it. The PR description says so and carries the
  review artefact instead: the `git diff -M100% --stat` proof of zero content change in the moved
  files, plus the list of the 55 edited files.
- Traps: segment-joined paths grep misses (`scripts/dev-runner.ts:565,566,747`,
  `scripts/setup-worktree.ts:15`). The RUST_LOG filter silently stops matching if missed
  (`dev-runner.ts:737`, `logger.rs:27,117`). Ignored `packages/app-lib/.env` and `.env.local` stay
  behind in existing checkouts: move them by hand or rerun setup, and `vp run down` first so Windows
  file handles do not block `git mv`. Primary stays on main with the old path. `.gitattributes` path
  lines do not follow renames. `bedrock/ports.toml` `upstream =` values name upstream paths: leave
  them. `packages/tooling-config/package.json:17` exports `app-lib.prettier.config.cjs` by file
  name: leave it. Java package `com.modrinth.theseus` and `theseus.jar` stay. Do not reformat moved
  files (pre-commit `vp staged` formats staged files, see
  `apps/app/src/api/oauth_utils/auth_code_reply/page.html`); verify the move has zero content diff
  with `git diff -M100% --stat`. Fix all 8 bedrock `#[path]` strings in this commit or bedrock stops
  compiling.

### F5 feat(contracts): the protocol crate exists, with limits and share rules as its first content

- Lane: foundation
- Depends on: F4
- Goal: `packages/contracts` exists with the Endpoint machinery, the full `ApiError`/`ErrorCode`,
  `PROTOCOL_VERSION`, the `client` and `server` features and its TS generator with a freshness test;
  limits and share rules are defined only there; api-client has `ContractClient` and the `backend`,
  `local` and `core(id)` getters; AGENTS.md's "One contract" bullet is true again.
- Scope: `packages/contracts/**` (new), `Cargo.toml`, `packages/bedrock/src/{lib,limits,disk}.rs`,
  `packages/sharing-client/src/{push/mod,pool/store,rules/mod}.rs`,
  `packages/sharing-storage/src/{lib,routes}.rs`,
  `packages/engine/src/api/instance/shared/publish.rs`,
  `apps/core/src/{backups/space.rs,state/servers/adapters/filesystem/archive.rs}`,
  `apps/backend/{package.json,src/sharing/pool.ts}`,
  `packages/api-client/{package.json,src/index.ts,src/core/abstract-client.ts,src/contracts/{client,errors,index}.ts,src/contracts/client.test.ts,src/modules/{bedrock,sharing}/*}`,
  `.prettierignore`, `.gitattributes`, `_typos.toml` (only if the generated TS trips it), root
  `vite.config.ts` (fmt ignore), `scripts/check-changed.ts`, `AGENTS.md` ("One contract" bullet),
  `pnpm-lock.yaml`. About 45 files.
- Adds / Changes / Deletes:
  - Adds `packages/contracts/{Cargo.toml,package.json}` (`@amberite/contracts`, private, exports
    `./ts/index.ts`), `src/lib.rs` (Endpoint with `DANGEROUS`, Method, Auth, body markers,
    `PROTOCOL_VERSION`), `error.rs` (full union per D-27, including `engine_unavailable`,
    `external_save_failed`, `sign_in_cancelled` and `unsupported`), `limits.rs`, `rules.rs`,
    `client.rs` (`call`, `call_stream`, `Remote`, `CallError`), `server.rs` (`IntoResponse`,
    `Req<E>`, `route::<E>()`) and `ts.rs` (writes `ts/contracts.ts`; test `ts_is_up_to_date`,
    ignored `write_ts`). Other modules are added by the slice that first fills them (D-18).
  - Adds `packages/contracts/ts/index.ts`, hand-written: `export * from './contracts'` now; the
    engine and App lines are added by A5a and A4 with their generated files.
  - Adds `packages/api-client/src/contracts/client.ts` (`ContractClient`, `ApiCallError` with
    `status`, `code`, `description`), getters on `AbstractModrinthClient`: `backend` (transport from
    AuthConfig and `accountApiUrl`), `local` and `core(id)` (transports injected later); vitest for
    URL, query and body building and error decoding.
  - Changes every limits importer to `contracts::limits` (app-lib publish.rs, sharing-client push
    and pool/store, sharing-storage lib and routes, Core space.rs and archive.rs; bedrock `disk.rs`
    imports contracts while bedrock lives); sharing-client rules consts to `contracts::rules`; the
    backend imports `MAX_SHARE_BYTES` and `ALLOWANCE_BYTES` from `@amberite/contracts`; api-client
    re-exports it.
  - CI: `cargo test -p contracts` runs inside the `rust` job (its filter includes
    `packages/contracts/**`); `check-changed.ts` `reachedBy` gains `packages/contracts` for
    `@modrinth/app-frontend` and `@amberite/backend`.
  - AGENTS.md "One contract": every call between the App and a backend is an Endpoint whose TS is
    generated into `@amberite/contracts` and called through `packages/api-client`; values Rust owns
    are generated the same way (D-112 carve-out).
  - Deletes `packages/bedrock/src/limits.rs`, `packages/api-client/src/modules/bedrock/limits.ts`,
    `packages/api-client/src/modules/sharing/rules.ts` and the rules generator test
    (`sharing-client/src/rules/mod.rs:215-252`), the backend's own limit consts, the two
    `.prettierignore` lines naming them; adds `packages/contracts/ts/` to `.prettierignore`, the fmt
    ignore and `linguist-generated` in `.gitattributes`.
- Green check:
  `SQLX_OFFLINE=true cargo test -p contracts -p sharing-client -p sharing-storage -p copal -p bedrock`;
  `pnpm --filter @amberite/backend typecheck` and `test`;
  `vp test run packages/api-client/src/contracts`; `check:changed`; `vp run up 1 2`, in the bridge
  tab open a shared instance's publish modal (size limit text) and a Core server's share config list
  (extensions).
- Risk: medium. New workspace package wiring and the shapes every lane builds on.
- Traps: the backend duplicates the limits at `apps/backend/src/sharing/pool.ts:14,16`.
  `.gitignore:59` ignores any folder named `generated`, so the output folder is `ts/`. `cargo test`
  in `apps/core` never reached other crates.
  `packages/ui/src/composables/server-sharing/config-files.ts:1` imports the rules from
  `@modrinth/api-client`, so keep the re-export. `.prettierignore:21` and
  `packages/api-client/src/index.ts:41-42` name the old files. Enumerate ErrorCode from the
  backend's 26 kinds and 20 codes, Core's `ErrorCode` (14), the store's `Code` (11) and the engine's
  error `code` strings in `packages/engine/src/error.rs`; keep engine strings byte for byte (D-28).
  `2fa_required` at `apps/backend/src/accounts/index.ts:372` is an HTTP 200 body, not an error code.

### F6 refactor(shared): bedrock's disk and net code lives in sharing-client

- Lane: foundation
- Depends on: F5
- Goal: `disk.rs` and `net/{address,tls,ip,error_body}` live in sharing-client; nothing imports
  `bedrock::disk` or `bedrock::net`; the one fingerprint check is `sharing_client::net::tls`.
- Scope: `packages/bedrock/{Cargo.toml,src/disk.rs,src/net/**,src/lib.rs}`,
  `packages/sharing-client/{Cargo.toml,src/lib.rs,src/disk.rs,src/net/**,src/backend/**,src/pool/store.rs,tests/backend_status.rs}`,
  `packages/sharing-storage/src/{lib.rs,pull.rs}`,
  `apps/core/src/{backups/space.rs,network/mod.rs,access/pairing.rs,state/servers/adapters/{filesystem/archive.rs,network/hosts.rs}}`,
  `apps/app/{Cargo.toml,src/api/cores/{mod,tls}.rs}`. About 20 files.
- Adds / Changes / Deletes: moves 5 files into `packages/sharing-client/src/{disk.rs,net/}`; adds
  `nix`, `windows-sys`, `rustls` to sharing-client and `sharing-client` to `apps/app`; rewrites
  imports; deletes the bedrock originals and deps bedrock no longer uses.
- Green check:
  `SQLX_OFFLINE=true cargo test -p sharing-client -p sharing-storage -p copal -p bedrock`;
  `cargo check -p theseus_gui`; `check:changed`; `vp run up 1 2`, pair or open the dev Core from the
  bridge tab and load its server list (the App's pinned TLS path).
- Risk: low. A move with import rewrites.
- Traps: `packages/bedrock/Cargo.toml:49-54` platform deps must follow `disk::free`.
  `apps/app/src/api/cores/tls.rs:2,6` is the only App user. `cargo shear` runs in CI, so drop deps
  bedrock no longer uses. Nothing inside bedrock uses `net`, so the move is safe once limits moved
  (F5).

### F7 chore: merge main into the integration branch before the lanes fork

- Lane: foundation
- Depends on: F6
- Goal: the integration branch contains main's latest upstream syncs, with upstream app-lib changes
  in `packages/engine` and ported files handled per `docs/internals/upstream.md`.
- Scope: whatever main changed; counted edits are conflict resolutions, usually under 30 files.
- Adds / Changes / Deletes: one merge commit `git -c merge.directoryRenames=true merge origin/main`
  on `restructure/07-sync-main`; conflicts resolved by upstream.md; lockfiles regenerated; `git rm`
  of any resurrected `ported` path.
- Green check: `git ls-files packages/app-lib` is empty; `check:changed`;
  `SQLX_OFFLINE=true cargo test --workspace --exclude toasty --locked`; `vp run up 1`, the App
  loads. Merge with a merge commit, never squash.
- Risk: medium. Upstream changes may touch files just renamed.
- Traps: main's `upstream-sync.ts` and `upstream.json` are the old shape; take ours. CodeRabbit may
  skip a PR over 300 files; say so in the description. A modify/delete on a deleted file means
  upstream changed behaviour that must be ported, not restored.

### A1 chore(engine): the engine starts without the legacy importer or one-time layout migrations

- Lane: A-engine-app
- Depends on: F7
- Goal: engine startup runs no layout migration or legacy import, and dead state fields are gone.
- Scope: `packages/engine/src/state/{mod.rs,dirs.rs,legacy_converter.rs,settings.rs}`,
  `packages/engine/src/state/content_store/commands/migration.rs`,
  `packages/engine/src/api/instance/{icon.rs,synced_packs/**}`, `upstream.json`. About 10 files.
- Adds / Changes / Deletes:
  - Deletes `State.restart_after_pending_update` (`state/mod.rs:123,357`) and
    `DirectoryInfo::global_handle_if_ready` (`state/dirs.rs:26-28`).
  - Deletes `state/legacy_converter.rs`, its call (`state/mod.rs:307`) and `Settings.migrated`; adds
    its `ported` entry (deleted on purpose).
  - Deletes `content_store::migrate` (`state/mod.rs:152`), the `migrate_instance_copies` loop
    (`:157-165`), `synced_packs::migrate_store` (`:176`), `migrate_legacy_icons` (`:214`),
    `Settings::migrate` (`state/settings.rs:368`) and their call sites.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`; `vp run up 1`, in the
  bridge tab open the Library and an instance's content tab, install a mod from Modrinth and see it
  listed.
- Risk: medium. One of the "migrations" could be a live pass.
- Traps: before deleting `migrate_instance_copies` (`content_store/commands/migration.rs`), confirm
  no current install path writes plain instance copies that only this loop moves into the store; if
  one does, keep it as an engine maintenance task and say so in the PR.
  `game_locales_layout_version` (`migration.rs:92`) gates a folder relocation new installs never
  need. The working tree may hold the other agent's spike (`state/spike_engine.rs`, edits in
  `state/mod.rs`, `state/cache.rs:1649`): start from the integration branch, not that tree.

### A2 refactor(engine): the engine builds without the global, and two engines run side by side

- Lane: A-engine-app
- Depends on: A1
- Goal: `State::new(settings_dir) -> Arc<State>` builds a complete engine with no global; two run in
  one process and each sees only its own files. `State::init` remains a thin wrapper for the App.
- Scope:
  `packages/engine/src/state/{mod.rs,cache.rs,instances/watcher.rs,instances/commands/sync_content_files.rs}`,
  `packages/engine/src/api/instance/synced_options/game_options/locales/sources.rs`,
  `packages/engine/src/util/fetch.rs` (FetchSemaphore). About 10 files.
- Adds / Changes / Deletes:
  - Adds `State::new`: DB, content store with `recover`, `recover_interrupted_jobs`, semaphores,
    process manager, watcher, then the maintenance loop (cache purge, store cleanup),
    `watch_instances_init`, `monitor_persisted_processes`, `refresh_all_instances`, all holding
    `Weak<State>` (D-32).
  - Adds `State::start_desktop(self: &Arc<Self>)` (Discord, friends socket, synced options
    reconcile, locale indexer, `ModrinthCredentials::refresh_all`, account origin binding);
    `State::init` = resolve dir, `get_or_try_init(State::new)`, `start_desktop` once.
  - The watcher is two-phase: `init_watcher()` returns the watcher and receiver; `State::new` spawns
    the loop with a `Weak` (removes `watcher.rs:173`).
  - `CachedEntry` FileHash takes `instances_dir` through `get_file_hash_many` (2 callers); the
    stale-while-revalidate spawn (`cache.rs:1192`) clones the pool and an `Arc`'d semaphore.
  - Adds test `two_engines_list_their_own_content` in `packages/engine/src/state/`.
- Green check: `SQLX_OFFLINE=true cargo test -p engine --lib two_engines`;
  `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`; `vp run up 1`, the Library loads,
  Discord and friends status work, editing a file in an instance's `mods/` updates the content tab.
- Risk: medium. Startup order changes shape.
- Traps: `state/mod.rs:157-253` tasks borrow `&'static Arc<State>`; `watcher.rs:170-173` spawns
  before State exists; `cache.rs:1649` FileHash calls `State::get()`, which with no global hangs and
  with another engine as global hashes in that engine's folder and silently drops files (spike). A
  call reaching `State::get()` with no global hangs instead of failing, so wrap the test in
  `tokio::time::timeout`. Tests must call `engine::install_crypto_provider()` first.
  `create_instance.rs:136` awaits `reconcile_instance_synced_options`, which calls `State::get()`,
  so register test instances through the row adapters as the spike did. `sync_content_files.rs:357`
  wakes the global locale indexer: wake the one on the engine passed in.

### A3 refactor(engine): events go to a sink the host creates

- Lane: A-engine-app
- Depends on: A2
- Goal: every emit goes through an `EventSink` the host created and subscribed to; a headless engine
  never panics on a loading bar; the App forwards the sink to its Tauri Channel unchanged.
- Scope: `packages/engine/src/event/**`, every `emit_*` and `init_loading` call site in
  `packages/engine/src/**`, `apps/app/src/main.rs`, `apps/app/src/api/utils.rs`
  (`progress_bars_list`), `apps/app-frontend/src/generated/app-events/**` (generated). About 70
  files.
- Adds / Changes / Deletes:
  - Adds `EventSink { tx: broadcast::Sender<AppEvent>, loading_bars }`; `State::new` takes it;
    `emit_*` take `&EventSink`; `LoadingBarId` holds a sink clone for its drop.
  - `initialize_state` creates the sink, subscribes, then builds State, and forwards postcard frames
    to the Channel.
  - Deletes `EVENT_STATE`, `EventState::{init,get,try_get}`, the `cli` feature and `indicatif`. The
    unused `AdsConsentRequired` variant stays (upstream's; reported in Z3).
  - `get_main_window` and the locale `app.emit` stay behind the `tauri` feature until A28 and A29.
  - Adds a test: a loading bar with no subscriber does not panic, and an instance event reaches a
    subscriber.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `cargo test -p theseus_gui`;
  `check:changed`; `vp run up 1`, in the bridge tab install a modpack and watch progress and the
  loading bar complete; launch an instance and see live logs.
- Risk: medium. A broadcast can drop under bursts where the Channel never did.
- Traps: log lines are an event each (`state/process.rs:753,770`): size the channel for bursts
  (4096) and log `Lagged` (A5a turns lag into `Resync`). `event/emit.rs:66,110` and
  `event/mod.rs:237` are the panicking `expect`s. `progress_bars_list` (`utils.rs`) reads the sink's
  loading bars; it becomes an engine route in A13. `util/fetch.rs:661` emits loading through the
  `LoadingBarId` it was given. The frontend is C's until MPa; only the generated bindings change
  here (regenerate, do not hand-edit).

### A4 feat(app): the App serves a local HTTP API, starting with boot info

- Lane: A-engine-app
- Depends on: A3, C2 (MPb), F5
- Goal: the App binds `127.0.0.1` with a secret path prefix, the D-63 guard and CORS; the native
  window and bridge tabs learn `localBase` synchronously; `GET /health` and `GET /app/boot` answer
  from the moment the listener binds; `/app/boot` replaces `get_amberite_dev_config`, `get_os`,
  `is_dev`, `are_updates_enabled`, `should_disable_mouseover` and the `getVersion` and plugin-os
  reads.
- Scope:
  `apps/app/src/{main.rs,dev.rs,server/{mod,guard,boot,tests}.rs,server/endpoints/{mod,boot}.rs,server/local-init.js}`,
  `apps/app/Cargo.toml` (axum), `apps/app/build.rs`, `apps/app/capabilities/plugins.json`,
  `apps/app/tauri.conf.json` (CSP), `apps/app/package.json`, `packages/contracts/ts/{app,index}.ts`
  (`app.ts` generated, one line in `index.ts`),
  `apps/app-frontend/src/{main.js,app/runtime/dev-config.ts,platform/{local,os}.ts,helpers/utils.js,composables/macCssFix.js,platform/modrinth-client.ts}`,
  `apps/app-frontend/package.json`. About 28 files.
- Adds / Changes / Deletes:
  - Adds `server/{mod,guard,boot}.rs`: `LocalServer::bind(port, secret)` before `tauri::Builder`
    (dev config `localPort`/`localSecret`, which C2 already passes; otherwise `:0` and a random
    secret), the guard (D-63: secret, Host, same-origin refusal, Origin, WS Origin) and CORS, and
    `Server-Timing` in debug builds. `/health` and `/app/boot` are Endpoints in
    `apps/app/src/server/endpoints/` (D-16), served from bind time (D-60).
  - Adds `server/local-init.js`, injected by an init-script plugin: it sets
    `window.__AMBERITE_LOCAL__` only in the top frame of the `main` window and only when
    `location.origin` is one of the App's origins (D-61), the same guard `api/webview-logs.js` uses,
    so the `signin` window on Microsoft's page never receives the secret.
  - Adds `platform/local.ts` (`localBase` from the init global or `#local=` fragment, kept in
    `sessionStorage`; `localFileUrl`; `isNativeWindow`) and wires `client.local`'s transport; the
    Modrinth client is `TauriModrinthClient` in the native window and `GenericModrinthClient` in a
    tab.
  - `platform/os.ts` reads the cached boot result synchronously; `main.js` awaits `/app/boot` (a
    local round trip that never waits on the engine) before importing `App.vue`.
  - CSP: `connect-src` gains `http://127.0.0.1:*` and `ws://127.0.0.1:*`.
  - Deletes the five commands, their `build.rs` names and the plugin-os JS and Rust dependency.
- Green check: `SQLX_OFFLINE=true cargo test -p theseus_gui` (`server/tests.rs`: no secret gives
  404, bad Host 421, `Sec-Fetch-Site: same-origin` 403, foreign Origin 403, allowed Origin gets CORS
  headers, a WS upgrade without Origin is refused; `/app/boot` answers before the engine is set);
  `check:changed`; `vp run up 1`, open the printed bridge URL (it carries `#local=` since C2) and
  check the version in settings and the OS-dependent window controls; `vp run up 1 --show-windows`,
  the native window starts and shows the version; in its devtools `window.__AMBERITE_LOCAL__` is
  set, and after starting Microsoft sign-in the same check in the `signin` window gives `undefined`.
  M1 (cold start to state ready) is no worse than C1's baseline.
- Risk: medium. It is the new security boundary; CORS and preflight must match WebView2 and
  WKWebView.
- Traps: `main.js:19` awaits the dev config before importing App, and `accountApiUrl` is read once
  at `modrinth-client.ts` construction, so boot must resolve first. A native window at
  `tauri://localhost` calls `http://127.0.0.1` cross-origin, so preflight is required (macOS
  unverified; flag it). Never log the secret. `apps/app/src/api/cores/proxy.rs:199`
  `cfg!(feature = "browser-bridge")` belongs to B until MPc and stays until C4. plugin-os reads are
  synchronous at setup time (`appearance-settings.vue:22`, `pages/instance/worlds/index.vue:606`).
  Sign-in (`dev::sign_in`) runs inside `initialize_state` (`main.rs:30-40,113`) until A10b. The
  bridge injects its own script, so no Tauri init script reaches a bridge tab: it relies on
  `#local=`. A plugin init script runs in every webview, which is why `local-init.js` guards on
  label, frame and origin.

### A5a feat(app): app events arrive as JSON over one local WebSocket

- Lane: A-engine-app
- Depends on: A4
- Goal: `/events` carries every engine and App event as JSON through the App's own socket loop; the
  Tauri `Channel` and its forwarder are gone; a lagging subscriber gets one `Resync` and the UI
  refetches; startup is otherwise unchanged (the Rust `initialize_state` command still starts the
  engine, now without a Channel).
- Scope: `apps/app/src/{main.rs,server/{mod,events}.rs,server/endpoints/events.rs}`,
  `packages/engine/src/event/{mod,emit}.rs`,
  `apps/app-frontend/src/{providers/setup/app-events.ts,helpers/state.ts,App.vue,features/settings/syncing/game-settings-modal/use-labels.ts}`,
  `packages/contracts/ts/{engine,app,index}.ts` (generated, plus the engine line in `index.ts`),
  `upstream.json`. About 22 files.
- Adds / Changes / Deletes:
  - The engine's `event/mod.rs` enum is `engine::Event`, JSON-tagged (D-77), with `FilesChanged`
    never serialized; the App adds `AppEvent` (Friend, Notification, OnboardingChecklist,
    InstanceGroupsChanged, Command with `ApproveCore`, GameOptionLocalesUpdated, Progress, OpEnded,
    Resync) in `apps/app/src/server/events.rs`. Each gains `cfg_attr(test, derive(TS))`; the
    engine's generated file starts here (D-16, D-18).
  - Adds the App's socket loop: subscribe to the sink, forward every engine and App event except
    `FilesChanged`, and on `Lagged` send `Resync` and carry on (D-36, D-78). No subscribe protocol.
  - `app-events.ts` decodes JSON; on `Resync` it invalidates every query. TS
    `initialize_state(events)` opens `/events`, then calls the Rust `initialize_state` (no Channel
    argument).
  - The raw `amberite-command` and `game-option-locales-updated` emits become `AppEvent` variants.
  - Deletes the Channel forwarder and the postcard decode path in `app-events.ts`. The postcard
    types, codec and serde-binhum stay, unused at runtime, until A5b.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui` (socket test: a subscriber
  receives an instance event as JSON; `FilesChanged` is not sent; a lagged receiver gets `Resync`);
  `check:changed`; `vp run up 1`, in the bridge tab launch an instance and watch process and log
  events, install a mod and watch job progress, trigger an `amberite://` deep link with the App
  running. M1 is no worse than A4's.
- Risk: high. Every live feature rides on it.
- Traps: `App.vue:727` awaits `initialize_state` before anything else. `Notification` is a JSON
  string today; it becomes a `Value`. `emit_command` focuses the main window
  (`event/emit.rs:174-180`); skip it when there is no window. Log bursts share the socket (doc
  Watch). `once` has 0 callers; leave it. `install/model.rs` is in the B window's list only after
  MP1; this slice is before it.

### A5b chore(engine): postcard, serde-binhum and the event codec are gone

- Lane: A-engine-app
- Depends on: A5a
- Goal: nothing in the engine or App uses postcard or serde-binhum; the generated app-events
  bindings and their export chain are gone. Mechanical; no behaviour change.
- Scope:
  `packages/engine/src/{event/**,install/events.rs,install/model.rs,state/{process,onboarding_checklist,instance_types}.rs,api/pack/import/mod.rs,api/instance/synced_options/game_options/locales/mod.rs}`
  (attributes only), `packages/engine/Cargo.toml`, `apps/app/Cargo.toml`, `apps/app/package.json`,
  `apps/app/src/bin/export_app_events.rs`, root `Cargo.toml`, `packages/serde-binhum/**` (deleted),
  `apps/app-frontend/src/providers/setup/app-event-codec.ts`,
  `apps/app-frontend/src/generated/app-events/**` (deleted), `.gitignore:60-67`, `upstream.json`.
  About 20 changed files excluding generated.
- Adds / Changes / Deletes: deletes every `#[serde_binhum]` attribute, `app-event-codec.ts`,
  `generated/app-events/**`, the `export-app-events`/`export-ts` postcard chain,
  `bin/export_app_events.rs`, `export-app-events` from `apps/app/package.json:6`'s dev script
  features, `packages/serde-binhum` (added to upstream ignore) and the `.gitignore` allow-list
  lines.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `cargo shear`;
  `check:changed`; `git grep -n "serde_binhum\|export-app-events" -- ':!.plans'` prints nothing;
  `vp run up 1`, in the bridge tab install a mod and watch job progress.
- Risk: low. Deletions the compiler checks.
- Traps: `instance_types.rs:77` carries `cfg_attr(feature = "export-ts", ...)`: remove the attribute
  with the feature or `unexpected_cfgs` fails clippy; bedrock compiles the same file and keeps its
  own dormant `export-ts` feature, so keep `postcard-bindgen` as a workspace dependency until D10
  (D-104). `apps/app/package.json:6` keeps `browser-bridge` until C4.

### A6 refactor(engine): instance- and job-keyed statics live on their owners

- Lane: A-engine-app
- Depends on: A5b
- Goal: no process static is keyed by instance or job id; install runner entry points take the
  engine.
- Scope: `packages/engine/src/install/{control.rs,store.rs,runner.rs,recovery.rs}`,
  `packages/engine/src/state/{installs.rs,process.rs,instances/watcher.rs}`,
  `packages/engine/src/api/instance/shared/report.rs`, api callers of the runner. About 15 files.
- Adds / Changes / Deletes: `CONTROLS` (`install/control.rs:6`), `INSTALL_TARGETS` (`runner.rs:52`),
  `LIVE_JOBS` and `COMPLETION_NOTIFICATIONS` (`install/store.rs:14,111`) move into `Installs`;
  `LOG_BUFFERS` (`state/process.rs:60`) into `ProcessManager`; `CONTENT_SYNCS` (`watcher.rs:90`)
  into `FileWatcher`; `PUBLISH_REPORTS` (`api/instance/shared/report.rs:10`) onto State. The 8
  public `install/runner.rs` fns (`:226-516`) and recovery take `&Arc<State>`; their api callers
  pass `State::get()` for now.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`; `vp run up 1`, start an
  install, pause, resume and cancel it from the download manager.
- Risk: low. Pure moves.
- Traps: completion notifications are `Weak<Notify>` (`install/store.rs`); keep the Weak semantics.

### A7 refactor(engine): the host sets the Modrinth session and the backend credential

- Lane: A-engine-app
- Depends on: A6
- Goal: engine code never reads `modrinth_users` or a sharing global; fetch reads the Modrinth
  session and account endpoints from the engine; sharing reads the host-set backend credential
  (D-38).
- Scope: `packages/engine/src/util/fetch.rs`, every fetch caller (about 55 sites),
  `packages/engine/src/self_hosted/{accounts.rs,pool/mod.rs}`,
  `packages/engine/src/api/instance/shared/{mod,client,icons,install}.rs`,
  `packages/engine/src/{state,api}/mr_auth.rs`, `apps/app/src/dev.rs`. About 50 files.
- Adds / Changes / Deletes:
  - Adds `Sessions { modrinth, backend, accounts }` on State with `set_modrinth_session` and
    `set_backend_session` (`BackendSession { base_url, bearer, kind: User | CoreKey }`); the `exec`
    param of the fetch family (`fetch.rs:681-1192`) becomes `sessions: &Sessions`.
  - Deletes `ENDPOINTS` (`self_hosted/accounts.rs:12`); `route` and `may_send_credentials` read
    `Sessions.accounts`, passed into `State::new`.
  - Deletes `SHARED_CLIENTS_SESSION` (`api/instance/shared/mod.rs:37`); `share_backend()` reads
    `Sessions.backend`.
  - Writers: Modrinth login, logout, `set_active`, `refresh_all`, `bind_origin`, `sync_sharing` and
    dev sign-in set the sessions after writing the DB.
  - Extends the test at `self_hosted/accounts.rs:222`: the session is attached only to URLs
    `may_send_credentials` allows.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`; `vp run up 1 2`, sign in
  on scenario 1, publish a shared instance, accept it on scenario 2; sign out and see sharing calls
  rejected.
- Risk: medium. A missed writer leaves a stale token in memory.
- Traps: token refresh (`ModrinthCredentials::refresh_all`) and the foreign-origin deactivation in
  `bind_origin` (`self_hosted/accounts.rs:148-163`) must update `Sessions`. `fetch_mirrors` requires
  `exec: Copy` (`fetch.rs:1160`). This is the last A slice in the B-window paths before MP1; B9 and
  B10 build on it. Keep the backend credential a single host-set value: Core-key calls get the
  acting user as an argument later (D5), not through a second store.

### A8 refactor(engine): instance, pack and install api functions take the engine

- Lane: A-engine-app
- Depends on: A7
- Goal: `api/instance/**`, `api/pack/**` and every api function reached from non-api engine code
  take `&State`; Tauri commands for those modules get it from an App-managed `EngineHandle`.
- Scope: `packages/engine/src/api/{instance/**,pack/**,jre.rs,metadata.rs}`, the 85 `crate::api::`
  call sites in `install/`, `launcher/`, `state/`;
  `apps/app/src/api/{instance.rs,install.rs,import.rs,jre.rs,metadata.rs}`,
  `apps/app/src/{main.rs,engine_handle.rs}`. About 120 files.
- Adds / Changes / Deletes: each `let state = State::get().await?;` becomes a `state: &State` (or
  `&Arc<State>` where it spawns) parameter; adds `apps/app/src/engine_handle.rs` (managed handle
  that waits until startup sets it).
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `cargo test -p theseus_gui`;
  `check:changed`; `vp run up 1`, create an instance, install a modpack, add and remove a mod,
  launch and kill the instance.
- Risk: low. Mechanical and compiler-checked.
- Traps: install runner chains `api::instance::create/edit` (`install/runner.rs:624-817,1659`) and
  `install/shared_instance.rs:473,962,992,1107`; `launcher/mod.rs:146-1019` calls `api::jre` and
  `api::metadata`; recursive `Box::pin` calls need the parameter threaded.

### A9 refactor(engine): the remaining api functions take the engine

- Lane: A-engine-app
- Depends on: A8
- Goal: no `State::get()` remains anywhere.
- Scope: every other `packages/engine/src/api/**` module, `apps/app/src/api/*.rs` except `cores/**`,
  `apps/app/src/main.rs`, `apps/app/src/api/thumbnails.rs`. About 70 files.
- Adds / Changes / Deletes: the A8 change for settings, logs, process, worlds, files, cache, tags,
  users, reports, skins, auth, mr_auth, friends, onboarding, handler, debug_info.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `cargo test -p theseus_gui`;
  `check:changed`; `vp run up 1`, open settings, worlds, logs, skins and the friends panel.
- Risk: low.
- Traps: `apps/app/src/main.rs` calls `State::get()` for asset scopes and in the update-on-exit path
  (`main.rs:410-476`). `apps/app/src/api/cores/**` is B's until MPc and does not use State.

### A10 refactor(engine): the global is gone and State is Engine

- Lane: A-engine-app
- Depends on: A9
- Goal: `LAUNCHER_STATE`, `STATE_STARTUP`, `State::get` and `State::init` are gone; the type is
  `Engine`, built only through `Engine::new(EngineConfig)`. No behaviour change: the Rust
  `initialize_state` command still builds it.
- Scope: `packages/engine/src/**`, `apps/app/src/**` except `api/cores/**`; about 200 files,
  mechanical.
- Adds / Changes / Deletes: deletes `state/mod.rs:91-94` globals, `State::get`, `initialized`,
  `get_if_initialized`, `startup_complete`; renames `State` to `Engine`; bundles `State::new`'s
  arguments into `EngineConfig` (`settings_dir`, `accounts`, `events` for now); `initialize_state`
  calls `Engine::new` then `start_desktop` and stores the engine in the `EngineHandle`.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `cargo test -p theseus_gui`;
  `check:changed`; `vp run up 1`, cold start and use the Library. Merging this is MP1.
- Risk: low.
- Traps: name clashes with `base64::Engine` imports (`apps/app/src/api/oauth_utils/auth_seal.rs:18`)
  and axum's `State`; import `engine::Engine` explicitly. bedrock `#[path]`-includes engine model
  files; if a bedrock file names `State`, stop and coordinate with B (B owns bedrock) rather than
  editing it.

### A10b feat(app): the engine starts in App setup and initialize_state waits on the socket

- Lane: A-engine-app
- Depends on: A10
- Goal: App setup builds the engine (and runs dev sign-in) right after the listener binds; engine
  routes and `/events` wait for it while `/health` and `/app/boot` answer from bind (D-60); the TS
  `initialize_state(events)` resolves when `/events` opens; the Rust `initialize_state` command is
  gone (D-79).
- Scope: `apps/app/src/{main.rs,server/{mod,events}.rs,engine_handle.rs,dev.rs}`,
  `apps/app/build.rs`, `apps/app/capabilities/plugins.json`,
  `apps/app-frontend/src/helpers/state.ts`. About 12 files.
- Adds / Changes / Deletes: setup spawns `Engine::new` then `start_desktop`, sets the `EngineHandle`
  and opens the readiness gate; a startup error makes the gated routes answer `engine_unavailable`;
  deletes the `initialize_state` command and its `build.rs` name.
- Green check: `cargo test -p theseus_gui` (a gated route waits for readiness; `/app/boot` does not;
  a failed start answers `engine_unavailable`); `check:changed`; `vp run up 1`, cold start, the
  Library loads in the bridge tab and the loading screen shows while the engine starts; record M1
  against A4's: no worse.
- Risk: medium. Startup order changes.
- Traps: the window must not need the engine before it paints. `App.vue:727` still awaits
  `initialize_state`, now a socket open. Dev sign-in ran inside the command (`main.rs:30-40,113`);
  it moves into setup before the gate opens.

### A11 feat(app): local images load over HTTP instead of asset://

- Lane: A-engine-app
- Depends on: A10b
- Goal: icons, world icons, screenshots, thumbnails, cached remote icons and picked-file previews
  load from `/app/file` and `/app/thumbnail`; `convertFileSrc`, the asset protocol and Tauri scope
  widening are gone.
- Scope:
  `apps/app/src/{api/instance.rs,api/worlds.rs,api/utils.rs,api/thumbnails.rs,main.rs,desktop/files.rs,server/endpoints/files.rs}`,
  `apps/app/tauri.conf.json`, `apps/app/build.rs`,
  `apps/app-frontend/src/{helpers/instance.ts,app/shell/title-bar/action-bar.vue,App.vue,components/ui/download-manager/use-download-manager.ts,composables/use-image-thumbnail.ts,platform/adapters/icon-cache.ts,providers/setup/file-picker.ts,helpers/worlds.ts,platform/local.ts}`.
  About 20 files.
- Adds / Changes / Deletes: adds `LocalFile`, `Thumbnail` and `CacheRemoteIcon` App routes (D-66;
  `cache_remote_icon` returns a path, so it is an App route per D-52); files under hash-named caches
  are served `immutable`, others with an mtime-and-length ETag and `no-cache`; Rust listings return
  paths and helper bodies map them through `localFileUrl`; deletes `get_image_thumbnail`,
  `cache_remote_icon`, the `convertFileSrc` imports, `assetProtocol`, the `asset:`/`asset.localhost`
  CSP entries and the `FsExt` and asset scope calls (`main.rs:42-48`, `instance.rs:1077-1115`); adds
  `img-src` and `media-src` `http://127.0.0.1:*`. plugin-fs stays until A12, which converts its
  `readFile` callers.
- Green check: `cargo test -p theseus_gui` (route test: 304 on a matching ETag, `immutable` for a
  cached icon, 404 without the secret); `check:changed`; `vp run up 1`, in the bridge tab check
  instance icons in the Library, world icons, the screenshots page, a Modrinth project icon and a
  generated icon in the creation modal.
- Risk: medium. Many visual surfaces change at once.
- Traps: the regexes in `action-bar.vue:350` and `use-download-manager.ts:105` pass `asset:` and
  `tauri:` through; update them. `cache_remote_icon` returned an `asset://` URL via `utils.rs:342`.
  Confirm which cache folders name files by hash before marking them `immutable`; icons edited in
  place need the ETag to change, so use mtime plus length. 10 `convertFileSrc` call sites are
  synchronous (`helpers/instance.ts` `getInstanceIconUrl`); `localFileUrl` must stay synchronous.

### A12 feat(app): dialogs, links, folders and shortcuts are App routes, and window chrome works in a tab

- Lane: A-engine-app
- Depends on: A11
- Goal: every desktop feature in D-67, D-68 and D-52 is an App route called the same way from the
  native window and a tab; window chrome, drag and drop and the edit menu are gated by
  `isNativeWindow`; the shell no longer throws at bootstrap in a plain tab.
- Scope: `apps/app/src/desktop/{dialog,opener,shortcuts,fs,network,webview_log}.rs`,
  `apps/app/src/server/endpoints/desktop.rs`,
  `apps/app/src/api/{utils.rs,shortcuts/**,logs.rs,files.rs,minecraft_skins.rs,mod.rs}`,
  `apps/app/{build.rs,capabilities/plugins.json,package.json,Cargo.toml}`,
  `apps/app-frontend/src/platform/{dialog,opener,window,files}.ts`,
  `apps/app-frontend/package.json`,
  `apps/app-frontend/src/{main.js,App.vue,app/shell/title-bar/window-controls.vue,helpers/native-drag-drop.ts,helpers/utils.js,providers/setup/{file-drop,file-picker,image-viewer-editor}.ts,components/ui/screenshots-page/index.vue,helpers/skins.ts}`,
  `upstream.json`. About 28 files.
- Adds / Changes / Deletes: adds the dialog, opener, folder, shortcut, `fs/read`, `fs/stat`,
  `network/metered` (replacing `is_network_metered`) and `webview-log` routes; converts the three
  `readFile` callers (screenshots page, file picker, image viewer editor) to `fs/read`; deletes
  `open_path`, `highlight_in_folder`, `show_launcher_logs_folder`, `show_app_db_backups_folder`,
  `is_network_metered`, `instance_open_screenshot`, `instance_open_synced_options_folder`,
  `create_instance_shortcut`, `file_read_dragged_file`, `file_local_info`, `get_dragged_skin_data`,
  `logs_log_webview` and `toggle_decorations` (nothing calls it; it goes with the commands its
  plugin loses, D-37), plus JS `plugin-dialog`, `plugin-opener`, `plugin-fs`, Rust plugin-fs and
  their capability lines.
- Green check: `cargo test -p theseus_gui`; `check:changed`; `vp run up 1 --show-windows`, from the
  bridge tab open the content-add dialog (it appears on the desktop), add a jar, open the instance
  folder and an external link, create a shortcut, open the screenshots page and a picked image; in
  the native window minimize, maximize, close and drag a jar in; open the printed plain-tab URL and
  confirm nothing throws during bootstrap (unconverted panels may error).
- Risk: medium. Touches many upstream component files, with no logic change in them.
- Traps: `main.js:31` `getCurrentWindow()` at bootstrap throws in a tab. `window-controls.vue:111`
  and `App.vue:742` call `saveWindowState` before closing. The dialog needs the main window as
  parent only when it is visible. `App.vue:1823` link interception routes everything through
  `openUrl`. `packages/ui/.../console/layout.vue:174` reads `__TAURI_INTERNALS__`; it stays
  (developer decision 5). 44 seam files are byte-identical to upstream: touch only import lines.

### A13 feat(engine): engine routes behind Access serve the Library's instance reads

- Lane: A-engine-app
- Depends on: A12
- Goal: `engine::routes::<S>()` exists with `Access` and the `InstanceAccess<P>` extractor; the App
  mounts it with `AppAccess`; instance list, get and get-many, groups (6) and the loading-bar list
  come over HTTP as engine routes, recent icon configs (1) as an App route.
- Scope:
  `packages/engine/src/{access.rs,routes/{mod,instances,groups,loading}.rs,endpoints/{mod,instances,groups,loading}.rs,lib.rs}`,
  `packages/engine/src/state/{instances/model/*.rs,instance_types.rs}` (`cfg_attr(test, derive(TS))`
  lines only), `packages/contracts/ts/engine.ts` (generated),
  `apps/app/src/{server/mod.rs,server/endpoints/library.rs,library/routes.rs,api/instance.rs,api/utils.rs,api/mod.rs,main.rs}`,
  `apps/app/{build.rs,capabilities/plugins.json}`,
  `apps/app-frontend/src/{helpers/instance.ts,helpers/instance-groups.ts,helpers/state.ts,platform/app-lib/icons/**,helpers/types.d.ts}`,
  `upstream.json`. About 27 files.
- Adds / Changes / Deletes:
  - Adds `engine::access` (`Access` with `caller` and `require` returning the granted `Permission`,
    `Caller`, `Permission { View, Edit, Owner }`; D-81) and the `InstanceAccess<View|Edit|Owner>`
    extractor; `routes` requires `Arc<Engine>: FromRef<S>` and `Arc<dyn Access>: FromRef<S>` and
    exposes each handler as a `pub fn` (D-39, D-84); the App's `AppAccess` allows all. Route tests
    with a fake `Access` that denies and one that allows.
  - Engine Endpoints in `packages/engine/src/endpoints/` over the engine's own types (D-16) for
    instance reads, groups and `ListLoadingBars` (replacing `progress_bars_list`; live updates stay
    `Loading` events); recent icon configs as an App route; helper bodies call `client.local`; wire
    types become re-exports (D-74).
  - Deletes those commands, their `build.rs` names and `ported` entries for removed upstream command
    code.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui` (route tests assert the JSON
  field names the helpers send); `check:changed`; hand parity (D-8) for `instance_list`,
  `instance_get`, `instance_get_many` and group reads on scenario 1; `vp run up 1`, in the bridge
  tab the Library lists instances with groups and icons, create and rename a group, the action bar
  shows a running install.
- Risk: medium. First engine routes; field naming between helper and route.
- Traps: Rust snake_case parameters arrive camelCase over invoke today (`helpers/cache.js:4`), so
  Endpoint JSON must keep the field names the helper sends. `instance_rows.rs:716-741` joins groups,
  icon config and sync preferences; the read still includes `synced_options` until A38.
  `groups.rs:128` updates `instances`. `helpers/instance.ts` has 78 invokes: convert only the read
  functions here. The TS derive lines are additive attributes in upstream files; add nothing else
  there. Recent icon configs still read engine.db until A38 moves the table to app.db (D-48).

### A14 feat(app): settings, the opening command, onboarding and hidden instances come over HTTP

- Lane: A-engine-app
- Depends on: A13
- Goal: `settings_get/set` and `cancel_directory_change` (App routes, still backed by the engine
  crate's Settings until A36), `store_usage/cleanup/set_cache_limit` (engine routes),
  `get_opening_command`, onboarding (2) and hidden instances (2) come over HTTP.
- Scope: `apps/app/src/{settings,desktop,onboarding,library}/routes.rs`,
  `apps/app/src/server/endpoints/**`,
  `apps/app/src/api/{settings.rs,utils.rs,onboarding_checklist.rs,instance.rs,mod.rs}` (hidden
  instances are `instance_get_hidden_ids` and `instance_set_hidden` in `instance.rs:1574-1590`),
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs}`,
  `packages/engine/src/{routes,endpoints}/store.rs`,
  `apps/app-frontend/src/{helpers/settings.ts,helpers/state.ts,helpers/onboarding-checklist.ts,platform/app-lib/{hidden-instances,onboarding-checklist}/**}`,
  `upstream.json`. About 20 files.
- Adds / Changes / Deletes: routes and helper bodies as in Goal; `RunEvent::Opened` stores the
  opening command in `AppState.desktop` and emits `AppEvent::Command`; deletes the commands and
  capability entries.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`; hand parity
  for `settings_get`, `get_opening_command`, the onboarding and hidden reads; `vp run up 1`, in the
  bridge tab change a setting and reload, hide and unhide an instance, check store usage in
  settings.
- Risk: medium. Settings writes touch launch and synced options.
- Traps: `settings::get/set` take the synced-options lock and reconcile fullscreen
  (`api/settings.rs:31-60`); keep that until A29. The shared-library cache
  (`api/instance/shared/library.rs`) is in the B window; it converts in A34. macOS
  `RunEvent::Opened` code is in `apps/app/src/macos`: keep the edit to the emit call and flag it.

### A16 feat(engine): tags, metadata and the Modrinth cache come over HTTP

- Lane: A-engine-app
- Depends on: A14
- Goal: `helpers/{tags,metadata,cache}.js` (25 commands) call engine routes over the engine.db
  cache.
- Scope: `packages/engine/src/routes/{tags,metadata,cache}.rs`,
  `packages/engine/src/endpoints/{tags,metadata,cache}.rs`,
  `apps/app/src/api/{tags,metadata,cache}.rs` (deleted),
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs,src/api/mod.rs}`,
  `apps/app-frontend/src/helpers/{tags,metadata,cache}.js`, `upstream.json`. About 12 files.
- Adds / Changes / Deletes: Endpoints, handlers, helper bodies; deletes the three plugins.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`; hand parity
  on every read; `vp run up 1`, in the bridge tab browse Modrinth, open a project page, filter by
  loader and version.
- Risk: low.
- Traps: the `cache` macro expands 2 templates into 16 commands; count from `build.rs`.
  `cache_remote_icon` moved to an App route in A11. Keep the camelCase field names the helpers send.

### A17 feat(engine): logs and processes come over HTTP

- Lane: A-engine-app
- Depends on: A16
- Goal: `helpers/{logs,process}.js` (11 commands) call engine routes; `process_wait_for` has no
  caller, so it goes with the plugin and its engine function stays (D-37).
- Scope: `packages/engine/src/routes/{logs,process}.rs`,
  `packages/engine/src/endpoints/{logs,process}.rs`, `apps/app/src/api/{logs,process}.rs`,
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs}`,
  `apps/app-frontend/src/helpers/{logs,process}.js`, `upstream.json`. About 10 files.
- Adds / Changes / Deletes: as in Goal; `logs_log_webview` already moved in A12.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`; hand parity
  on log and process reads; `vp run up 1`, in the bridge tab launch an instance, read live and past
  logs, kill it.
- Risk: low.
- Traps: live log lines arrive as `Log` events on the socket, not through these routes.

### A18 feat(app): Java and launcher import come over HTTP

- Lane: A-engine-app
- Depends on: A17
- Goal: jre is engine routes except path-taking `jre_get_jre` and `jre_test_jre` (App routes);
  import discovery and `install_import_instance` are App routes in `apps/app/src/import/` (D-51).
- Scope: `packages/engine/src/routes/jre.rs`,
  `apps/app/src/{import/routes.rs,desktop/jre.rs,api/{jre,import}.rs}`,
  `packages/engine/src/endpoints/**`, `apps/app/src/server/endpoints/**`,
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs}`,
  `apps/app-frontend/src/helpers/{jre,import}.js`, `upstream.json`. About 12 files.
- Adds / Changes / Deletes: as in Goal; deletes the two plugins.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`; hand parity
  on jre reads; `vp run up 1`, in the bridge tab open Java settings (detected installs, max memory),
  test a JRE path; open Import and see detected launchers.
- Risk: low.
- Traps: `install_import_instance` calls `install/runner.rs:155`; it stays engine code called by the
  App route. `maxMemoryQueryOptions` is used by `App.vue` at startup.

### A19 refactor(app-frontend): user, preference and report helpers call Modrinth's client directly

- Lane: A-engine-app
- Depends on: A18
- Goal: `helpers/{users,user-preferences,reports}.ts` call upstream's labrinth modules; the `users`
  and `reports` commands and `packages/engine/src/api/{users,reports}.rs` are gone (D-50).
- Scope: `apps/app-frontend/src/helpers/{users,user-preferences,reports}.ts`,
  `apps/app/src/api/{users,reports}.rs`, `packages/engine/src/api/{users,reports,mod}.rs`,
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs}`, `upstream.json`. About 10 files.
- Adds / Changes / Deletes: helper bodies only; deletes 14 commands, two engine files (with `ported`
  entries).
- Green check: `cargo test -p theseus_gui`; `check:changed`; hand parity on `get_user`, followed
  projects and preferences; `vp run up 1`, in the bridge tab open a user profile, follow a project,
  report flow opens.
- Risk: low.
- Traps: `SelfHostedFeature` routes account paths to the backend and strips `Authorization` from
  other labrinth requests; requests that need the Modrinth session must go where it attaches it. The
  engine's users functions cached through `CachedEntry`; the frontend's query cache replaces it.

### A30 feat(app): game options, synced packs and screenshots come over HTTP

- Lane: A-engine-app
- Depends on: A19
- Goal: `helpers/{game-options,synced-packs}.ts` and the screenshot and synced-option parts of
  `helpers/instance.ts` call App routes in `apps/app/src/{synced_options,screenshots}/routes.rs`.
- Scope: those helpers, `apps/app/src/{synced_options,screenshots}/routes.rs`,
  `apps/app/src/api/instance.rs` (40 APP-DOMAIN commands), `apps/app/src/server/endpoints/**`,
  `apps/app/{build.rs,capabilities/plugins.json}`, `upstream.json`. About 16 files.
- Adds / Changes / Deletes: routes and helper bodies; screenshot save is raw bytes; deletes the
  commands.
- Green check: `SQLX_OFFLINE=true cargo test -p theseus_gui`; `check:changed`; hand parity on
  option, pack and screenshot reads; `vp run up 1`, in the bridge tab edit game settings, toggle a
  synced pack, open and delete a screenshot.
- Risk: medium.
- Traps: `game-options.ts` declares 20 wire types: re-export them under the same names (D-74).
  `instance.ts` keeps its other invokes until A33 and A34. The routes call the engine crate's
  synced-options code as A20 does; A29 later moves that code behind events without changing these
  routes. This slice runs before A15 so the plain tab starts without failing startup reads
  (`App.vue:412-424`).

### A15 feat(app): Minecraft and Modrinth sign-in run from App routes; the shell works in a plain tab

- Lane: A-engine-app
- Depends on: A30
- Goal: the `auth` and `mr-auth` plugins are App routes in `apps/app/src/accounts/routes.rs` calling
  the engine crate's functions; with the startup reads already converted (A13, A14, A16, A18, A19,
  A30), the App shell, Library, settings and sign-in work in the plain tab that `up` prints without
  errors. From here every converted screen is checked in the plain tab (D-7).
- Scope:
  `apps/app/src/{accounts/routes.rs,server/endpoints/accounts.rs,api/auth.rs,api/mr_auth.rs,main.rs,build.rs}`,
  `apps/app/capabilities/plugins.json`, `apps/app-frontend/src/helpers/{auth.js,mr_auth.ts}`,
  `upstream.json`. About 12 files.
- Adds / Changes / Deletes: `POST /app/accounts/minecraft/login` opens the native sign-in window on
  the desktop and answers `sign_in_cancelled` when it closes; Modrinth login runs today's listener
  and deep-link handoff; the cancel route; deletes all 13 `auth` and `mr-auth` commands. Helper
  names and signatures stay.
- Green check: `check:changed`; hand parity on `get_users`, `get_default_user`, `get_all`, `get`;
  `vp run up 1 --show-windows`, open the printed plain-tab URL: the shell, Library and Settings
  render and the console shows no failed startup call; start Microsoft sign-in from the tab, the
  sign-in window opens on the desktop, close it and the call rejects with `sign_in_cancelled`; run
  Modrinth sign-in once in the real flow or with computer use. Record M1 to M3 for the tab against
  C1's baseline.
- Risk: medium. The OAuth listener and deep-link handoff are timing sensitive.
- Traps: `auth.rs:42-80` polls a WebviewWindow URL. `oauth_utils/auth_code_reply/page.html` was
  reformatted by the pre-commit formatter: do not touch it. `mr_auth.rs` focuses the main window,
  which may not be visible. If a startup panel still calls `invoke`, list it in the PR with the
  slice that converts it.

### A20 feat(app): friends and skins come over HTTP

- Lane: A-engine-app
- Depends on: A15
- Goal: `helpers/{friends,skins}.ts` call App routes in `apps/app/src/{friends,skins}/routes.rs`;
  the domain code still lives in the engine crate until A35.
- Scope: `apps/app/src/{friends,skins}/routes.rs`, `apps/app/src/api/{friends,minecraft_skins}.rs`,
  `apps/app/src/server/endpoints/{friends,skins}.rs`,
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs}`,
  `apps/app-frontend/src/helpers/{friends,skins}.ts`, `upstream.json`. About 14 files.
- Adds / Changes / Deletes: skin texture upload and read are raw bytes (D-17); deletes the two
  plugins.
- Green check: `SQLX_OFFLINE=true cargo test -p theseus_gui`; `check:changed`; hand parity on friend
  and skin reads; `vp run up 1 2`, send, accept and remove a friend between the two plain tabs;
  change a skin on scenario 1.
- Risk: medium. Raw-bytes skins and presence.
- Traps: `number[]` skin bytes convert at the helper edge so signatures stay. Friend presence events
  now arrive as `AppEvent::Friend` on the socket (A5a).

### A21 feat(app): long file operations start, report progress as events and cancel by op id

- Lane: A-engine-app
- Depends on: A20
- Goal: debug export, external save, local upload and `file_save_as` start and return (D-70): `POST`
  registers a caller-made `op`, progress is `AppEvent::Progress`, the end is `AppEvent::OpEnded`,
  `DELETE /app/ops/{op}` cancels; the per-call Channels and window-label cleanup are gone; the
  `utils` plugin, whose last commands these are, is gone.
- Scope:
  `apps/app/src/{server/ops.rs,server/endpoints/ops.rs,desktop/{debug_export,external_save,upload}.rs,api/files.rs,api/utils.rs,api/mod.rs,main.rs,build.rs}`,
  `apps/app/capabilities/plugins.json`,
  `apps/app-frontend/src/{components/ui/download-manager/debug-info-export.ts,platform/adapters/{file-download,local-file-upload}.ts,platform/app-lib/files/commands.ts,pages/instance/files/index.vue}`,
  `packages/api-client/src/platform/xhr-upload-client.ts`. About 16 files.
- Adds / Changes / Deletes: the ops registry (start, progress, end, cancel); deletes
  `export_debug_info`, `cancel_debug_info_export`, `files_select_external`, `files_save_external`,
  `files_release_external`, `file_upload_local`, `file_cancel_local_upload`, `file_save_as`, the
  `.setup`/`.on_event` window hooks (`files.rs:18-44`, `utils.rs:24-38`), the `utils` plugin with
  `apps/app/src/api/utils.rs` (its other commands went in A4, A11, A12, A13 and A14) and its
  `build.rs` block.
- Green check: `cargo test -p theseus_gui` (start answers 202 before the op ends; the cancel route
  cancels and `OpEnded` carries `cancelled`; a failing save maps to `external_save_failed`);
  `check:changed`; `git grep -n "plugin:utils" apps/app-frontend/src` prints nothing;
  `vp run up 1 2`, in the plain tab export debug info and cancel one; with the dev Core paired,
  download a Core backup through the hosting files tab in the bridge tab (Core routing is
  `core_connect` until A24).
- Risk: medium.
- Traps: `file-download.ts:68-72` sends an explicit `Origin` to the Rust downloader; keep it
  accepted. `files_save_external` has its own error shape `ExternalSaveError {message, statusCode}`
  (`files.rs:438`); it becomes `ApiError` with `external_save_failed` and the status in the
  description. `XHRUploadClient.uploadLocalFile`
  (`packages/api-client/src/platform/xhr-upload-client.ts:199`) expects `{loaded, total}`: map
  `Progress` to it.

### A22 feat(app): updates are checked, downloaded and installed through App routes

- Lane: A-engine-app
- Depends on: A21
- Goal: D-71 is in place; no Tauri resource id is involved.
- Scope: `apps/app/src/{desktop/updater.rs,updater_impl.rs,updater_impl_noop.rs,main.rs}`,
  `apps/app/capabilities/updater.json` (deleted), `apps/app/tauri-release.conf.json`,
  `apps/app-frontend/src/{helpers/utils.js,App.vue}`, `apps/app-frontend/package.json`. About 9
  files.
- Adds / Changes / Deletes: deletes `get_update_size`, `enqueue_update_for_installation`,
  `remove_enqueued_update`, `set_restart_after_pending_update`, `restart_app`, the
  `invoke('plugin:updater|check')` and the `plugin-updater` JS dependency.
- Green check: `cargo check -p theseus_gui --features updater`; `cargo test -p theseus_gui`;
  `check:changed`; `vp run up 1`, settings shows no update with updates disabled;
  `POST /app/restart` restarts the dev App.
- Risk: medium. A real update can only be tested from a release build; flag it.
- Traps: `updater_impl.rs:19,66` use `Webview::resources_table`. Install-on-exit is in
  `RunEvent::Exit` (`main.rs:410-476`). `checkLinuxUpdates` uses plain `fetch` to Modrinth's bucket
  (`App.vue:1709`); leave it and report it.

### A23 refactor(app): the Core proxy lives in apps/app/src/cores

- Lane: A-engine-app
- Depends on: A22, B1 (MPc)
- Goal: `apps/app/src/api/cores/*` is at `apps/app/src/cores/*` with no other change.
- Scope: 5 files moved, `mod` lines in `apps/app/src/main.rs` and `apps/app/src/api/mod.rs`.
- Adds / Changes / Deletes: a pure `git mv`.
- Green check: `cargo test -p theseus_gui` (`cores/tests.rs`); `git diff -M100% --stat` shows pure
  renames.
- Risk: low.
- Traps: none beyond keeping the diff a pure rename.

### A24 feat(app): Cores are reached through one App proxy listener

- Lane: A-engine-app
- Depends on: A23
- Goal: D-80 and D-63 are in place: the four `core_*` commands are `/app/cores` routes on the local
  server; the proxy keeps one listener of its own for all Cores, on a different port, with the D-63
  guard and `CSP: sandbox` plus `nosniff` on every response; per-tunnel listeners and capabilities
  are gone.
- Scope: `apps/app/src/cores/{mod,proxy,tests}.rs`, `apps/app/src/server/{mod,guard}.rs`,
  `apps/app/src/server/endpoints/cores.rs`,
  `apps/app/{build.rs,capabilities/plugins.json,capabilities/dev.json,tauri-local.conf.json}`
  (`dev.json` deleted and its name dropped from `tauri-local.conf.json`),
  `apps/app-frontend/src/platform/modrinth-client.ts`. About 12 files.
- Adds / Changes / Deletes: deletes the `cores` plugin, `proxy::capability` per tunnel, the
  `allowed_origin` copy (the proxy uses `server/guard.rs`) and the tailscale `http` allow-list
  entry; keeps `http://127.0.0.1:*/*` in the `http` allow list, because the native window's
  `TauriModrinthClient` reaches the proxy through `tauriFetch`; the App's local `CoreError` values
  become `ApiError` with `invalid_input`.
- Green check: `cargo test -p theseus_gui` (`cores/tests.rs` on the proxy listener; new: `<img>`
  with no Origin passes, `Sec-Fetch-Site: same-origin` is refused, a WS with no Origin is refused,
  every response carries `Content-Security-Policy: sandbox` and `nosniff`, the WebView's
  `Authorization` is dropped and the session added, the proxy's origin is refused by the local
  server); `check:changed`; `vp run up 1 2`, pair the Core, and from the plain tab open the hosting
  panel's console, files and backups and upload a file; `vp run up 1 --show-windows`, open the same
  panel in the native window (the `tauriFetch` path).
- Risk: high. Security on the path to other people's machines.
- Traps: `proxy.rs:1-17` documents why the capability and Origin were required; rewrite it to the
  new guard and say why the proxy keeps its own origin. `modrinth-client.ts:49-67` is the only
  caller of `core_connect`. Core console sockets must still upgrade through it. `proxy.rs:199`
  `cfg!(feature = "browser-bridge")` stays until C4. The dev runner passes `tauri-local.conf.json`
  as `TAURI_CONFIG` on every App build (`scripts/dev-runner.ts:766-768`): an unknown capability name
  there fails the dev build.

### A25 refactor(app): the App resolves the data directory and finishes directory moves before the engine starts

- Lane: A-engine-app
- Depends on: A24
- Goal: the engine takes explicit paths and limits; identifier, `THESEUS_CONFIG_DIR`,
  settings-driven directory moves and DB backups run in the App before `Engine::new` (D-54).
- Scope: `packages/engine/src/state/{mod.rs,dirs.rs,dirs/move_app_directory.rs,db.rs,db_backup.rs}`,
  `packages/engine/src/logger.rs`, `apps/app/src/{main.rs,data_dir.rs}`,
  `apps/app/src/settings/routes.rs`. About 15 files.
- Adds / Changes / Deletes: `EngineConfig` gains `data_dir`, `instances_dir` (required), `logs_dir`,
  `db_path` (until A38), `max_concurrent_downloads`, `max_concurrent_writes`, `side`;
  `DirectoryInfo` loses `app_identifier`; adds `apps/app/src/data_dir.rs` (resolve, back up, run the
  pending move with the engine-crate function, read limits from settings); moves
  `move_launcher_directory` and `record_current_app_version` out of engine startup.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`; `vp run up 1`, change the
  app directory in settings, restart through the App, instances are at the new place; cancel a
  pending change.
- Risk: medium. The move runs at a different point in startup.
- Traps: the move emits `LoadingBarType::DirectoryMove` (`state/dirs.rs:284`), so the sink must
  exist before `Engine::new`. `move_app_directory.rs:550` reads `settings.prev_custom_dir` and the
  `processes` and `install_jobs` tables. `cancel_directory_change` only needs the identifier.

### A26 refactor(app): launches take credentials, launch defaults and game options from the App

- Lane: A-engine-app
- Depends on: A25
- Goal: engine launch reads no account, settings or synced-options data; one App launch function
  prepares them and calls the engine (D-47).
- Scope: `packages/engine/src/api/instance/run.rs`, `packages/engine/src/launcher/{mod.rs,args.rs}`,
  `packages/engine/src/install/diagnostics.rs`, `apps/app/src/launch.rs`,
  `apps/app/src/api/{instance,worlds}.rs`. About 12 files.
- Adds / Changes / Deletes: adds `LaunchCredentials`, `LaunchDefaults`, `LaunchRequest`,
  `ensure_installed`; `launch` replaces `run`/`run_credentials`; `args.rs:243-255` reads
  `LaunchCredentials`; deletes `launcher/mod.rs:1206-1224` (synced packs, game options, launcher
  overrides) and the `run.rs:279` skin flush, now in `apps/app/src/launch.rs` after
  `ensure_installed`; `install/diagnostics.rs:628-652` redacts by pattern and `minecraft_tokens`
  goes (D-43). Tests: arguments carry the given credentials; redaction removes a JWT and an
  `--accessToken` value.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`; `vp run up 1`, launch an
  instance with the scenario's Minecraft account, join a server from the worlds tab, open a
  singleplayer world; change memory in settings and see it in the launched process arguments.
- Risk: medium. Install, sync and launch must keep today's order.
- Traps: `launcher/mod.rs:766` reconciles synced options inside `install_minecraft`, which ran
  before the pre-launch sync, so the App calls `ensure_installed` before syncing.
  `credentials.maybe_online_profile()` uses `PROFILE_CACHE` (`state/minecraft_auth.rs:448`), which
  stays with the App's accounts. Quarantine and shared-availability checks (`run.rs:29-43`) stay in
  the engine.

### A27 feat(app): worlds and server joins come over HTTP

- Lane: A-engine-app
- Depends on: A26
- Goal: `helpers/worlds.ts` calls engine world routes; `start_join_server` and
  `start_join_singleplayer_world` are App routes over `apps/app/src/launch.rs`.
- Scope: `packages/engine/src/routes/worlds.rs`, `packages/engine/src/endpoints/**`,
  `apps/app/src/server/endpoints/**`, `apps/app/src/{api/worlds.rs,launch.rs,library/routes.rs}`,
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs}`,
  `apps/app-frontend/src/{helpers/worlds.ts,App.vue}`, `upstream.json`. About 12 files.
- Adds / Changes / Deletes: 17 world commands and the two joins; world icons come back as paths
  (A11).
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`; hand parity
  on world reads; `vp run up 1`, in the plain tab list worlds, rename one, join a server, open a
  singleplayer world.
- Risk: medium.
- Traps: `adapt_world_icon` in `worlds.rs` built `asset://` URLs; it is gone since A11. `App.vue`
  imports `start_join_*` for deep links.

### A28 refactor(app): Discord, friends status and the window follow process events

- Lane: A-engine-app
- Depends on: A27
- Goal: engine process and launch code never touches presence or the window; the App reacts to
  `Process` events (D-46).
- Scope: `packages/engine/src/launcher/mod.rs`,
  `packages/engine/src/state/{process.rs,mod.rs,presence.rs}`, `packages/engine/src/event/emit.rs`,
  `apps/app/src/{main.rs,reactions.rs}`. About 10 files.
- Adds / Changes / Deletes: deletes `launcher/mod.rs:1274-1297`, `state/process.rs:1025-1043`,
  `Engine.presence`, `get_main_window` and the window focus in `emit_command`; adds
  `apps/app/src/reactions.rs` (subscribed before `Engine::new`) and moves Discord and friends
  startup from `start_desktop` into the App.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`; `vp run up 1 2`, launch on
  scenario 1 and see the friend status on scenario 2 change; enable "hide on launch", the native
  window minimizes and returns on close.
- Risk: medium. Reactions now run asynchronously.
- Traps: `friends.rs:118` and `discord.rs:137` read `ProcessManager::get_all`, reachable through the
  engine. macOS window code compiles only on release builds: keep it untouched.

### A29 refactor(app): synced options, packs, screenshots, locales and onboarding follow instance events

- Lane: A-engine-app
- Depends on: A28, B10 (MP2)
- Goal: no engine-core file calls synced options, packs, screenshots, the locale indexer or
  onboarding; pack updates call `PackUpdateHooks`; the engine's `tauri` feature and dependency are
  gone (D-45).
- Scope: the 23 call lines (`install/runner.rs:997,1479`, `install/shared_instance.rs:344-365`,
  `launcher/mod.rs:766`,
  `state/instances/commands/{create_instance.rs:136,content_mutation.rs:172,remove_instance.rs:13-24,sync_content_files.rs:357}`,
  `state/instances/watcher.rs:125,252,357,385,393,431`, `state/process.rs:968`,
  `api/pack/install_mrpack.rs:1250-1266`, `api/instance/lifecycle.rs:114,127`,
  `api/settings.rs:31,48`),
  `packages/engine/src/state/{mod.rs,synced_options.rs,instance_locks.rs}`,
  `packages/engine/src/event/mod.rs`, `packages/engine/Cargo.toml`, `apps/app/Cargo.toml`,
  `apps/app/src/{reactions.rs,pack_hooks.rs}`, `apps/app/src/server/events.rs`,
  `.github/workflows/ci.yml` (guard). About 30 files.
- Adds / Changes / Deletes: `Instance::FilesChanged { paths }`; the locale indexer emits
  `AppEvent::GameOptionLocalesUpdated` from the App; `PackUpdateHooks` with the App implementation
  (`prepare_instance_update`, `capture_modpack_server_override`, `capture_game_options_pack_base`,
  `reconcile_instance_after_pack_update`); App reactions for Created, Synced, Edited, FilesChanged,
  Process Finished and InstallJob completed; deletes `Engine.synced_options`,
  `InstanceLocks.screenshots`, the engine's `tauri` feature and dependency; the ci.yml guard adds
  `-p engine -i tauri`.
- Green check: `SQLX_OFFLINE=true cargo test -p engine`; `cargo tree -p engine -e normal -i tauri`
  finds nothing; `check:changed`; `vp run up 1`, change a synced game option and see it reach a
  second instance; take an in-game screenshot and see it on the screenshots page; update a modpack
  and confirm a changed `options.txt` value survives.
- Risk: high. Ordering-sensitive desktop behaviour moves.
- Traps: `create_instance.rs:136` awaited the reconcile inline; it now runs after `Created`.
  `remove_instance.rs:13` held the synced-options lock during removal, so the App's reconciler must
  tolerate a vanished instance. `PackSyncWorker` and `GameLocaleIndexer` were started from
  `State::init`. `install/shared_instance.rs` is in the B window: this slice waits for B10.
  `content_mutation.rs:172` and `synced_packs/worker.rs:36,49` queue on the global indexer (spike).

### A31 feat(engine): the files tab and content storage read and write raw bytes over HTTP

- Lane: A-engine-app
- Depends on: A29
- Goal: `pages/instance/files/index.vue` (10 inline invokes) and `content-storage.vue` (3) go
  through helpers on engine file routes with raw bodies (D-17).
- Scope: `packages/engine/src/routes/files.rs`, `packages/engine/src/endpoints/files.rs`,
  `apps/app/src/api/files.rs`,
  `apps/app-frontend/src/{pages/instance/files/index.vue,components/**/content-storage.vue,platform/app-lib/files/**}`,
  `apps/app/{build.rs,capabilities/plugins.json}`, `upstream.json`. About 12 files.
- Adds / Changes / Deletes: file read, write, list, rename, delete and extract-zip
  (`file_extract_zip`, its dry run included) routes (`InstanceAccess<Edit>`); deletes the `files`
  plugin's remaining commands.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`;
  `vp run up 1`, in the plain tab open, edit and save `options.txt`, create and delete a folder,
  check content storage.
- Risk: medium. Raw bytes and paths.
- Traps: `file_read` returned `number[]`; convert at the helper edge. Path-taking variants stay App
  routes (D-52).

### A32 feat(engine): installs come over HTTP

- Lane: A-engine-app
- Depends on: A31
- Goal: `helpers/install.ts` (28 invokes) calls engine install routes.
- Scope: `packages/engine/src/routes/install.rs`, `packages/engine/src/endpoints/install.rs`,
  `apps/app/src/api/install.rs`, `apps/app-frontend/src/helpers/install.ts`,
  `apps/app/{build.rs,capabilities/plugins.json,src/main.rs}`, `upstream.json`. About 12 files.
- Adds / Changes / Deletes: 27 install routes; deletes the `install` plugin except
  `install_import_instance` (A18).
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`; hand parity
  on job reads; `vp run up 1`, in the plain tab create an instance from a modpack, watch progress,
  pause, resume, cancel; `App.vue`'s modpack preview works.
- Risk: medium.
- Traps: `helpers/install.ts:117-151` switches on `error.code`; engine codes must keep their strings
  (D-28). `install.ts` declares 15 wire types.

### A33 feat(engine): instance content and lifecycle come over HTTP, and play goes through the App

- Lane: A-engine-app
- Depends on: A32
- Goal: the content and lifecycle parts of `helpers/instance.ts` call engine routes; `instance_run`
  is an App route over `apps/app/src/launch.rs`; path-taking calls are App routes (D-52);
  `instance_content`, which nothing calls, goes with the plugin and its engine function stays
  (D-37).
- Scope: `packages/engine/src/routes/{content,lifecycle}.rs`, `packages/engine/src/endpoints/**`,
  `apps/app/src/server/endpoints/**`,
  `apps/app/src/{api/instance.rs,desktop/instance_paths.rs,launch.rs}`,
  `apps/app-frontend/src/helpers/instance.ts`, `apps/app/{build.rs,capabilities/plugins.json}`,
  `upstream.json`. About 20 files.
- Adds / Changes / Deletes: content list, add, toggle, update, remove, dependencies; create, edit,
  duplicate, delete; `get_full_path`, `get_mod_full_path`, `add_project_from_path`,
  `edit_icon(path)`, `export_mrpack(location)` as App routes; deletes the commands.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`; hand parity
  on content reads; `vp run up 1`, in the plain tab add a mod from Modrinth, toggle, update and
  remove it, export an mrpack, play the instance.
- Risk: high. Launch and content are the core flows.
- Traps: `get_dependencies_as_content_items` takes upstream `Labrinth` types. Credentials were read
  inside `run.rs:46` before A26. `instance.ts` has 78 invokes in all; this slice must not touch the
  sharing part.

### A34 feat(engine): sharing comes over HTTP and the last engine command is gone

- Lane: A-engine-app
- Depends on: A33, B10 (MP2)
- Goal: the sharing parts of `helpers/instance.ts` and `platform/app-lib/shared-instances` call
  engine routes; share publish uses an op (D-70); the shared-library cache and
  `instance_share_can_current_user_use` are App routes; the `instance` plugin and `EngineHandle` are
  gone, leaving only window chrome on Tauri.
- Scope: `packages/engine/src/routes/sharing.rs`, `packages/engine/src/api/instance/shared/**`,
  `packages/engine/src/endpoints/**`, `apps/app/src/server/endpoints/**`,
  `apps/app/src/{api/instance.rs,library/routes.rs,engine_handle.rs,main.rs}`,
  `apps/app-frontend/src/{helpers/instance.ts,platform/app-lib/shared-instances/**}`,
  `apps/app/{build.rs,capabilities/plugins.json}`, `upstream.json`. About 20 files.
- Adds / Changes / Deletes: sharing routes over the engine's sharing functions, returning B10's
  contract types; deletes the engine's hand-written sharing types that kept the Tauri commands'
  shapes until now (`api/instance/shared/types.rs`, `client.rs:17-93`, with
  `apps/app/src/api/install.rs`'s `SharedInstanceInstallPreview` users), the remaining `instance`
  commands, the plugin, `apps/app/src/engine_handle.rs` and `apps/app/src/api/` if empty.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui -p sharing-client`;
  `check:changed`; hand parity on share reads; `vp run up 1 2`, scenario 1 publishes a share,
  scenario 2 installs it, scenario 1 pushes an update and scenario 2 sees the diff;
  `git grep -n "invoke(" apps/app-frontend/src` lists only window chrome.
- Risk: high. Sharing across two Apps.
- Traps: share publish had a per-call Channel (`helpers/instance.ts:808`); it becomes an op. The
  shared library cache is a `CachedEntry` keyed by the user id the App passes (D-48).

### A40 refactor(app-frontend): our Core and notification screens call contracts directly

- Lane: A-engine-app
- Depends on: A34 (and MP2, which A34 already needs)
- Goal: our own screens call `client.backend` and `client.core(id)` Endpoints with generated types;
  `packages/api-client/src/modules/archon/cores/v1.ts` (ours, a per-route hand client), the `Cores`
  namespace in `archon/types.ts` and the Amberite-added `labrinth/notifications/v3.ts` are gone; our
  app-frontend callers of the Amberite sharing additions use `client.backend`, leaving
  `packages/api-client/src/sharing/` only for `packages/ui`'s calls (D-90, D-102).
- Scope:
  `apps/app-frontend/src/{App.vue,components/ui/shared-instances/SharedInstancePublishModal.vue,components/ui/shared-instances/shared-instance-invite-handler/use-shared-instance-invite-handler.ts,features/cores/{add-core-modal.vue,choose-core-modal.vue,use-cores.ts},features/hosting/use-server-play.ts,features/servers/{create-server-from-instance-modal.vue,linked-servers.ts,local-servers.vue},features/settings/account/cores-settings.vue,pages/instance/worlds/index.vue}`,
  `packages/api-client/src/{modules/archon/cores/**,modules/archon/types.ts,modules/labrinth/notifications/v3.ts,modules/index.ts,index.ts,types/client.ts,features/core-session.ts,sharing/**}`.
  About 22 files.
- Adds / Changes / Deletes: each caller switches to the Endpoint it was wrapping (the B4 cores
  routes, B6 Core-native routes, B7 notification routes); deletes `archon/cores/v1.ts` and its test,
  the `Cores` namespace, `labrinth/notifications/v3.ts`, the module registrations, and the
  `packages/api-client/src/sharing/` methods no `packages/ui` file calls. Comments naming
  `archon.cores_v1` (`types/client.ts:56-57`) follow.
- Green check: `vp test run packages/api-client`; `check:changed`;
  `git grep -n "cores_v1\|Archon.Cores\|notifications_v3" -- packages apps` prints nothing outside
  `packages/ui` comments; `vp run up 1 2`, in the plain tab add a Core and choose it when publishing
  a share, open Settings > Account > Cores, accept a shared-instance invite from a notification on
  scenario 2, read and dismiss a notification.
- Risk: medium. Many of our screens change call sites at once; the types catch shape drift.
- Traps: `archon/servers/v0.ts`, `archon/actions/v1.ts` and `archon/server-users/v1.ts` reference
  `Cores` types through Amberite edits; they go back to upstream in D5 and D9, so until then replace
  each reference with the generated type, nothing more. `packages/ui` files must not change: grep
  `packages/ui` for every name before deleting it.

### A35 refactor(app): accounts, skins, friends and Discord code moves into apps/app

- Lane: A-engine-app
- Depends on: A40
- Goal: Minecraft and Modrinth accounts (with `self_hosted/accounts.rs` and `oauth_utils`), skins,
  friends (with `tunnel.rs`) and Discord presence live in
  `apps/app/src/{accounts,skins,friends,discord}`. Their startup (credential refresh, origin
  binding) leaves `State::start_desktop`. No behaviour change.
- Scope: moved files from `packages/engine/src/{state,api,self_hosted}/**` and
  `apps/app/src/api/oauth_utils/**`, plus import fixes in both crates, `apps/app/Cargo.toml`
  (ariadne, async-tungstenite, p256 move with their users), `upstream.json` (`ported` entries).
  About 40 files, mostly moves.
- Adds / Changes / Deletes: `git mv` and `crate::` path fixes only.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`;
  `git diff -M --stat` shows the moves; `vp run up 1 2`, sign in, friends and a skin change work in
  the plain tab.
- Risk: low. Compiler-checked moves.
- Traps: these tables still live in the engine's single DB until A38: the moved code reaches the
  pool through the engine handle for now. Do not reformat moved upstream files.

### A36 refactor(app): synced options, screenshots, library, onboarding, settings and desktop code moves into apps/app

- Lane: A-engine-app
- Depends on: A35
- Goal: synced options (game options, command history, synced servers, synced packs, locale
  indexer), screenshots, hidden instances, onboarding, desktop settings, deep links and the logger's
  desktop part live in their `apps/app/src/<domain>` folders. `State::start_desktop` (A2) is gone:
  its last parts (synced-options reconcile, locale indexer) start from App setup. No behaviour
  change.
- Scope: moved files from `packages/engine/src/{state,api}/**`, import fixes, `upstream.json`. About
  50 files, mostly moves.
- Adds / Changes / Deletes: `git mv` and path fixes only.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`; `check:changed`;
  `vp run up 1`, synced options, screenshots, hidden instances and settings work in the plain tab.
  `git grep -n start_desktop packages/engine apps/app` prints nothing.
- Risk: low.
- Traps: `game_options.rs` and the locale indexer are upstream files; record each in `ported`.
  Settings' engine half (limits) already reaches the engine through `EngineConfig` (A25).

### A37 refactor(app): desktop domains call the backend through contracts

- Lane: A-engine-app
- Depends on: A36, B8
- Goal: the App's accounts, friends and sessions code calls `contracts::backend::{accounts, social}`
  with `call::<E>()`; `self_hosted::accounts::route()` URL rewriting is gone (D-58).
- Scope: `apps/app/src/{accounts,friends}/**`, `apps/app/src/dev.rs` (`/_dev/session` through
  `backend::dev`), `packages/engine/src/self_hosted/accounts.rs` (deleted or reduced to the engine's
  endpoints), `apps/app/Cargo.toml` (contracts `client` feature). About 15 files.
- Adds / Changes / Deletes: `call::<E>()` callers; deletes `route()` (`self_hosted/accounts.rs:51`)
  and the hand response structs it parsed.
- Green check: `SQLX_OFFLINE=true cargo test -p theseus_gui -p engine`; `check:changed`;
  `vp run up 1 2`, sign out and in, friend request between tabs, a user profile loads.
- Risk: medium. A routing miss sends credentials to the wrong host.
- Traps: `may_send_credentials` still decides which Modrinth fetches carry the session; keep its
  test. The engine still needs `accounts` endpoints for Modrinth paths (D-33).

### A38 refactor(engine): engine.db and app.db are separate fresh schemas

- Lane: A-engine-app
- Depends on: A37
- Goal: the engine opens `data_dir/engine.db` with engine tables only; the App opens `app.db` in its
  settings dir; nothing joins or cascades across them (D-55); AGENTS.md's "Test data" section says
  so.
- Scope: `packages/engine/migrations/**`, `packages/engine/.sqlx/**` (generated),
  `packages/engine/src/state/{db.rs,instances/adapters/sqlite/instance_rows.rs,instances/model/**,instances/commands/get_instance.rs}`,
  `apps/app/migrations/**`, `apps/app/.sqlx/**` (generated),
  `apps/app/src/{main.rs,db.rs,reactions.rs,library/routes.rs}`,
  `apps/app-frontend/src/helpers/instance.ts`, `scripts/scenario-state.ts` and its test,
  `.gitattributes`, `packages/engine/.cargo/config.toml`, `AGENTS.md` ("Test data" section only,
  D-112 carve-out), `upstream.json`. About 42 files plus generated data.
- Adds / Changes / Deletes: deletes the 55 upstream migrations (`ported` to the two init files) and
  adds `packages/engine/migrations/<ts>_init.sql` and `apps/app/migrations/<ts>_init.sql` with
  E-13's table lists, except that `recent_instance_icon_configs` moves to app.db (D-48); groups,
  icon configs and `join_log` stay in engine.db; hidden instances go to app.db; `instance_rows.rs`
  drops sync preferences (`:514-579,913,1291-1314`) and screenshot sources (`:480,498`); engine
  `Instance` drops `synced_options` and the helper merges the App's sync-preference route (D-56);
  the App deletes its rows on `Instance::Removed`; drops `EngineConfig.db_path`; the share-history
  `SCHEMA` still runs on engine.db (D-42); `scenario-state.ts` normalizes engine tables in
  engine.db; the four `.gitattributes` `!eol` lines go; the sqlx regeneration commands go in a
  comment in `packages/engine/.cargo/config.toml` next to `SQLX_OFFLINE` (D-41); AGENTS.md "Test
  data" names engine.db and app.db per scenario and core.db plus engine.db for the Core. Tests:
  engine.db's table set equals an explicit allowed list (D-48), which D1 reuses for the Core;
  `Removed` clears the App rows.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p theseus_gui`;
  `vp test run scripts/scenario-state.test.ts`; `check:changed`; `vp run down`, delete this
  worktree's `.data/scenarios` (D-109), then `vp run up 1 2` on fresh scenarios, sign in, create an
  instance, set a synced option, delete the instance and confirm its App rows are gone.
- Risk: high. Every query moves database and old data folders stop working.
- Traps: an old `app.db` with upstream `_sqlx_migrations` rows fails the new migrator, so only fresh
  scenarios work from here; every later worktree resets (D-109). Runtime `sqlx::query()` calls
  (`instance_rows.rs` group CRUD, `install/diagnostics.rs`) are not checked by `.sqlx`; grep them.
  Four synced tables had no CASCADE (`20260819...sql:22,97,113,121`, `20260911...sql:10`); the App
  cleanup must cover them. bedrock `#[path]`-includes engine model files: model changes must still
  compile in bedrock (bedrock passed to A at MP2). `.prettierignore` still names the old migrations
  path; leave it to D. Lane C must not edit AGENTS.md's "Test data" section while this slice is
  open.

### A39 build(engine): the engine links no desktop crate, and CI keeps it that way

- Lane: A-engine-app
- Depends on: A38
- Goal: `packages/engine/Cargo.toml` lists no desktop-only dependency and CI fails if one returns
  (D-59).
- Scope: `packages/engine/Cargo.toml`, `Cargo.lock`, `.github/workflows/ci.yml`. About 4 files.
- Adds / Changes / Deletes: deletes `discord-rich-presence`, `ariadne`, `async-tungstenite`, `p256`,
  `winreg` from the engine if A35 and A36 left them unused (`cargo shear` confirms); the ci.yml
  guard adds those crates for `-p engine`.
- Green check: `cargo tree -p engine -e normal | grep -E "tauri|discord|ariadne"` prints nothing;
  `SQLX_OFFLINE=true cargo test -p engine`; `check:changed`.
- Risk: low.
- Traps: Gradle still runs in `packages/engine/build.rs`; the Core needs the `.env` copy (F2 did
  it). `windows` may be a real engine dependency for process handling; guard only crates that are
  desktop-only.

### B1 fix(protocol): one ApiError for the backend, Core, the store and their clients

- Lane: B-protocol
- Depends on: F7
- Goal: every server we own answers errors as `{ error: ErrorCode, description }` from F5's enum;
  every Rust and TS reader switches on the typed code; the backend's `LOCAL_DEV` CORS accepts
  `*.localhost`.
- Scope: `apps/core/src/**` (22 files using `ErrorCode`), `apps/core/src/api/typescript.rs`,
  `packages/api-client/src/modules/copal/generated.ts` (generated),
  `packages/sharing-storage/src/{lib,routes,tickets}.rs`,
  `packages/sharing-client/src/{backend/error,backend/mod,pool/store,push/mod,net/error_body}.rs`,
  `apps/core/src/{access/pairing,api/content}.rs`, `apps/app/src/api/cores/mod.rs`,
  `apps/backend/src/**` (about 12 files, including `index.ts:12-28` CORS),
  `packages/api-client/src/{features/core-session.ts,modules/shared-instances/errors.ts,modules/copal/access/v1.test.ts}`.
  About 55 files.
- Adds / Changes / Deletes:
  - Core and store use `contracts::ApiError`; `Unauthorized`, `InvalidRequest`, `Internal` become
    `InvalidCredentials`, `InvalidInput`, `InternalError`; the backend's `ApiError` takes
    `(status, code, description)` and `errorResponse` sends the exact code as `error`.
  - String matches become enum matches (`pool/store.rs:536-541`, `push/mod.rs:333-334`,
    `content.rs:955-961`, `pairing.rs:321-327`); `core-session.ts:53` checks `invalid_credentials`;
    `SharedInstancesApiError.code` reads `error`; the App's `cores/mod.rs:87` parses `ApiError` and
    its two local `"invalid_request"` strings become `invalid_input`.
  - Deletes `apps/core/src/api/error.rs`, the store's `Code`/`StoreError`,
    `packages/sharing-client/src/net/error_body.rs`, the `ErrorCode`/`ApiError` decls in Core's
    `typescript.rs`.
- Green check:
  `SQLX_OFFLINE=true cargo test -p contracts -p sharing-client -p sharing-storage -p copal`;
  `cargo check -p theseus_gui`; `pnpm --filter @amberite/backend test`; `check:changed`;
  `vp run up 1 2`, in the bridge tab sign out of a Core session and reopen the panel (re-auth),
  invite an existing member (the toast shows), push a share to a full store if reproducible. Merging
  this is MPc.
- Risk: medium. A wire change on three servers at once; misses show as wrong toasts.
- Traps: Core's role tests compare codes (`apps/core/src/api/tests.rs:216,677`).
  `shared-instance-context.ts:250` compares `error.code`. Backend error kinds built across several
  lines escape a one-line grep: type the constructor first and let tsc list them. Keep
  sharing-client's public error type names, or the change leaks into engine files (lane A).

### B2 refactor(backend): one fresh D1 schema with contract table names

- Lane: B-protocol
- Depends on: B1
- Goal: `apps/backend/migrations` is one file creating the final 28 tables with D-88's names and the
  `version_hash` column; behaviour is unchanged; the backend's black-box tests are `*.api.test.mjs`.
- Scope: `apps/backend/migrations/*`,
  `apps/backend/src/{sharing/index,sharing/pool,cores/storage,cores/servers,common/notifications}.ts`,
  `apps/backend/tests/*.mjs` (renamed from `*.contract.test.mjs`, D-91), `apps/backend/package.json`
  (test globs), `apps/backend/README.md`. scripts.md (C's) is not touched. About 30 files (16
  migration files deleted).
- Adds / Changes / Deletes: adds `0001_schema.sql` with D-88's full rename list (`shared_members` to
  `shared_instance_users`, `shared_versions` to `shared_instance_versions`, `shared_files` to
  `shared_instance_files`, `shared_links` to `shared_instance_invites`, `shared_reports` to
  `shared_instance_reports`, `shared_servers` to `hosted_servers`, `server_players` to
  `hosted_server_players`; `root` to `version_hash`); changes the 97 SQL-bearing lines; deletes the
  16 old files; renames the four test files.
- Green check: `pnpm --filter @amberite/backend test`; `vp run down`, delete this worktree's
  `.data/backend` only, then `vp run up dev:backend` and `vp run up 1 2`; sign in through dev
  sign-in, create and open a shared instance.
- Risk: low. Mechanical; the local backend data reset is expected.
- Traps: `tests/worker.mjs:49` applies migrations by sorted filename and splits on `;`, so no `;`
  inside comments or strings. `0005_sharing.sql` and `0011_pool.sql` open with `DROP TABLE`; drop
  that history. Wrangler's local `d1_migrations` will try `0001_schema.sql` on an old
  `.data/backend` and fail on `users`, so the reset is required, and every later worktree based on
  B2 resets too (D-109): scenario data that references old backend ids is unusable, so lanes A and C
  rebuild the two-scenario sharing setup with scripts.md's regeneration steps before checks that
  share (A7, A20, A28, A34). Say so in the PR so the lanes see it at rebase. `.data/backend` keeps
  the old `root` column name in nothing after this.

### B3 refactor(sharing-client): no ProjectType, ModLoader or bedrock content

- Lane: B-protocol
- Depends on: B2
- Goal: sharing-client imports nothing from `bedrock::state` or `bedrock::content`; the Core-only
  `consume` lives in `apps/core/src/share_pack.rs` (D-103); `Side` is `contracts::Side` (D-24).
- Scope: `packages/sharing-client/src/{diff/mod,install/modpack,install/mod,rules/mod,lib}.rs`,
  `packages/sharing-client/src/consume/mod.rs -> apps/core/src/share_pack.rs`,
  `packages/contracts/src/{lib,side}.rs`,
  `apps/core/src/{api/content,state/servers/commands/install_instance,state/servers/commands/write_shared_pack,main}.rs`,
  `packages/sharing-client/Cargo.toml`, `packages/sharing-storage/Cargo.toml`,
  `docs/internals/shared-rust.md`. About 10 files.
- Adds / Changes / Deletes: `ProjectType::from_name`/`iterator` (`diff/mod.rs:96`,
  `install/modpack.rs:75`, `rules/mod.rs:167`) become `ContentType` plus a folder table in `rules`;
  `sharing_client::install::Side` moves to `contracts::Side`, with a `pub use` left in
  `sharing_client::install` so the engine's import compiles until A edits it; drops bedrock from
  sharing-client's and sharing-storage's manifests; moves `consume`; updates shared-rust.md's crate
  graph.
- Green check:
  `SQLX_OFFLINE=true cargo test -p sharing-client -p sharing-storage -p copal -p contracts`;
  `cargo check -p engine`; `check:changed`; `vp run up 1 2`, follow a share on a Core server (server
  pack install) and view a share diff in the App.
- Risk: low. A move plus a type swap with tests on both sides.
- Traps: `ProjectType` serde accepts the `shader` alias (`instance_types.rs:149`); keep
  `shaderpacks` and `shader`. `ContentType` has `Plugin` and `ModPack`, which have no instance
  folder. `consume` is called at `apps/core/src/api/content.rs:30`. Keep sharing-client's public
  signatures; if one must change, the engine call site is lane A's file: stop and coordinate. The
  `Side` re-export goes in B10, which owns the engine's sharing paths and switches the last
  importer.

### B4 refactor(backend): Core-facing backend routes are contract Endpoints

- Lane: B-protocol
- Depends on: B3
- Goal: `contracts::backend::cores` (14 routes, `Keys`, the sign-in wait socket) and
  `contracts::tickets` are the only definitions; the backend's cores handlers are typed through the
  new router; Core, the store and api-client call them through `call::<E>()` and `client.backend`.
- Scope: `packages/contracts/src/{backend/cores,tickets}.rs`, `packages/contracts/ts/contracts.ts`
  (generated),
  `apps/backend/src/{index,common/router,common/http,cores/index,cores/storage,cores/tickets,cores/sign-in,cores/servers}.ts`,
  `apps/backend/tests/{core-access,core-sync,admin}.api.test.mjs`,
  `apps/core/src/{access/sync,access/pairing,access/keys,access/tickets,access/owed,api/backend,api/typescript,storage}.rs`,
  `packages/sharing-storage/src/{keys,tickets,pull}.rs`,
  `packages/api-client/src/modules/{archon/cores/v1.ts,archon/types.ts,copal/access/v1.ts,copal/generated.ts}`.
  About 32 files.
- Adds / Changes / Deletes: the backend router `route(E, parse, handle)` and per-endpoint `parse`
  for cores; `sync.rs` keeps its failure policy (`sync.rs:111`) but reads `CallError` status;
  `pairing.rs` uses `call`; `Keys::fetch` (`keys.rs:212`) uses `call::<Keys>`; `GET /cores`,
  approve, lookup, delete and ticket move to `client.backend` behind the same module methods for
  now, and the `Cores` namespace in `archon/types.ts` becomes re-exports of the generated types, so
  the types have one definition while A40 moves our screens off the module (D-102). Deletes the
  bodies in `apps/core/src/api/backend.rs` (`StateReport`, `Grant`, `Remove`, `StorageReport`,
  `StorageAnswer`, `PullOrder`, `TicketClaims`), the store's `Claims`/`PublicKeys`, backend `Holder`
  and the `parseServers`/`natural` duplicates, and their `typescript.rs` decls.
- Green check: `SQLX_OFFLINE=true cargo test -p contracts -p sharing-storage -p copal`;
  `pnpm --filter @amberite/backend test` and `typecheck`; `check:changed`; `vp run up 1 2`, open the
  Cores list, approve a Core sign-in code, grant and revoke a user on a Core server, see
  `POST /core/state` 204 and `POST /core/storage` in `.data/dev.log`.
- Risk: medium. Ticket claims are read by three parties; a field rename breaks Core sign-in.
- Traps: `/core/auth/poll` returns 410 `code_expired` and the TS maps status to `ApprovalFailure`
  (`archon/cores/v1.ts:371-380`); switch it to codes. The callers of `archon/cores/v1.ts` are lane
  A's files (`App.vue`, `features/cores/**`): keep the module's method names and signatures.
  `/core/keys.json` is a static asset (`wrangler.jsonc` assets); the Endpoint only documents it.
  `POST /core/access` answers `{id,username,avatar_url}`, which `sync.rs:407` ignores. `OwedCall`
  JSON persisted in `backend_owed` now nests contract types (D5 makes core.db fresh).
  `TICKET_HEADER` is exported from `copal/access/v1.ts` via `api-client/src/index.ts`.

### B5 refactor(core): Core's state struct is CoreState

- Lane: B-protocol
- Depends on: B4
- Goal: Core's `api::AppState` is `CoreState`; no behaviour change.
- Scope: `apps/core/src/**` (about 40 files).
- Adds / Changes / Deletes: renames `api::AppState` (`apps/core/src/api/mod.rs:64`) and every
  `State<Arc<AppState>>`.
- Green check: `SQLX_OFFLINE=true cargo test -p copal`; `check:changed`.
- Risk: low.
- Traps: the name clashes with the App's `AppState` in docs and grep. Import `base64::Engine as _`
  at `apps/core/src/access/sessions.rs:5` before `engine::Engine` arrives in D1.

### B6 feat(core): Core-native routes are contract Endpoints and /health reports the protocol

- Lane: B-protocol
- Depends on: B5
- Goal: Core-native routes (access, sessions, pairing, activity, update, storage, eula, logs,
  machine, performance, insight, console commands, restarts, java, memory, flags) are
  `contracts::core` Endpoints with unchanged paths; `/health` answers
  `Health { protocol, version }`; `client.core(id)` works over today's Core routing and checks the
  protocol on first use; the CLI calls through one generic caller; api-client's hand-written
  `copal/*` client modules are gone.
- Scope: `packages/contracts/src/core/**`, `packages/contracts/ts/contracts.ts` (generated),
  `apps/core/src/api/{typescript.rs,health.rs,access.rs,sessions.rs,pairing.rs,activity.rs,update.rs,storage.rs,eula.rs,logs.rs,insight.rs,performance.rs,console.rs,startup.rs}`,
  `apps/core/src/cli/**`,
  `packages/api-client/src/{modules/copal/**,modules/index.ts,features/core-session.ts,core/abstract-client.ts,modules/archon/{cores,server-users,actions}/v1.ts}`,
  `apps/app-frontend/src/{features/settings/account/cores-settings.vue,pages/instance/layout.vue}`,
  `.prettierignore`. About 60 files.
- Adds / Changes / Deletes: the contracts::core types and Endpoints; `cli/call.rs`; the
  `ProtocolMismatch` check and the three messages (D-30) in `cores-settings.vue` and where a Core
  server opens; switches the message-text match at `instance/layout.vue:391` to codes. Deletes the
  hand-written `copal/*` modules and the Core-native decls in `typescript.rs`; the share, diff,
  history and edited-file decls stay in a trimmed `typescript.rs` and `copal/generated.ts` until D5.
- Green check: `SQLX_OFFLINE=true cargo test -p copal -p contracts`; `check:changed`; `vp run up 1`,
  open Settings > Account > Cores (storage numbers load), open a Core server's Access page (members
  list; invite needs a ticket), run `copal servers list` against the dev Core; point the App at a
  Core whose `/health` omits `protocol` and see it treated as version 1.
- Risk: medium. Wire types for the access and storage UI change.
- Traps: `typescript.rs:25-92` also declares share, diff, history and edited-file types: leave them
  for D5 and do not move them twice. `packages/ui` share-diff imports `Copal.*` types: api-client
  keeps exporting those names (D-102), so `packages/ui` does not change. `cli/tests.rs` (11 tests)
  round-trips through the real socket and stays the guard. `cli/servers.rs:147,502,578,587,596` and
  `cli/backups.rs:215` hard-code paths that D9 reshapes; only the Core-native ones move here.

### B7 refactor(social): labrinth-shaped social routes and the friends socket are contract Endpoints

- Lane: B-protocol
- Depends on: B6
- Goal: `contracts::backend::social` (29 routes, raw avatar and icon bodies, `SharedInstanceChange`,
  the `FriendsSocket` upgrade over ariadne's message types) types the backend's social handlers;
  friend refusals have codes and keep labrinth's exact text; upstream pages, `packages/ui` included,
  work unchanged (D-29, D-89).
- Scope: `packages/contracts/{Cargo.toml,src/backend/social.rs}`,
  `packages/contracts/ts/contracts.ts` (generated), `packages/ariadne/{Cargo.toml,src/**}` (a `ts`
  feature and two `cfg_attr` lines),
  `apps/backend/src/{index,social/index,social/preferences,social/socket,common/notifications}.ts`,
  `packages/api-client/src/{contracts/labrinth-compat.ts,modules/labrinth/notifications/v3.ts}`,
  `apps/app-frontend/src/composables/use-shared-instance-event.ts`. About 18 files.
- Adds / Changes / Deletes: social Endpoints and types in labrinth shape; `already_friends`,
  `cannot_friend_self`, `own_friend_request`, each sent with labrinth's description text byte for
  byte (`packages/ui/.../access/access.vue:373` matches "you are already friends with this user",
  the same text as `modrinth/main`); `labrinth-compat.ts`; social handlers through the router with
  `/v2`, `/v3`, `/_internal` stripped; `FriendsSocket` with the backend handler typed by the
  generated message types; `labrinth/notifications/v3.ts` (ours) keeps its methods with bodies over
  `client.backend` until A40 moves its callers (`App.vue`, `use-shared-instance-invite-handler.ts`,
  lane A's files); deletes backend `Presence`, `Connection` and notification hand types, the hand
  `SharedInstances.LiveEvent` users, and the always-empty
  `/user/{id}/projects|organizations|collections` stubs after a grep finds no caller (D-90).
- Green check: `pnpm --filter @amberite/backend test` and `typecheck`;
  `cargo test -p contracts -p ariadne`; `check:changed`;
  `git diff origin/refactor/restructure-repo -- packages/ui` is empty for this slice;
  `vp run up 1 2`, send, accept and remove a friend between scenarios 1 and 2; add yourself as a
  friend from the Core access page and see the right message; open a profile; change avatar; read
  and dismiss a notification; friend presence updates live; a member change on a shared instance
  refreshes live.
- Risk: medium. Upstream pages consume these shapes; the compat file and the text test guard them.
- Traps: `/friends` answers `id` = recipient, `friend_id` = sender (`social/index.ts:30`).
  `PATCH /user/{id}/icon?ext=` takes raw bytes capped at 512 KiB. `/users?ids=` carries JSON in the
  query string. The socket handler (`social/socket.ts:197`) runs before every other handler
  (`index.ts:42`). App Rust still reaches these routes through URL rewriting until A37: do not
  change paths. Add a backend test asserting the three refusal descriptions equal labrinth's
  strings. ariadne is upstream: the edit is one feature line and two additive attributes.

### B8 refactor(accounts): labrinth-shaped account and dev routes are contract Endpoints

- Lane: B-protocol
- Depends on: B7
- Goal: `contracts::backend::accounts` (25 routes) types the account handlers; `backend::dev`
  (`/_dev/session`, `/_dev/outbox`, served only under `LOCAL_DEV`) and the backend's `/health` are
  Endpoints too; `SelfHostedFeature` routes by a matcher generated from the accounts, social and
  sharing path tables (D-85, D-90).
- Scope: `packages/contracts/src/backend/{accounts,dev}.rs`, `packages/contracts/ts/contracts.ts`
  (generated), `apps/backend/src/{index,accounts/index,common/auth,dev}.ts`,
  `apps/backend/tests/admin.api.test.mjs`,
  `packages/api-client/src/{features/self-hosted.ts,contracts/labrinth-compat.ts,modules/labrinth/users/v3.ts,modules/labrinth/sessions/v2.ts}`,
  `scripts/self-hosted-routing.test.ts`. About 14 files.
- Adds / Changes / Deletes: accounts Endpoints including the `2fa_required` login answer as a union
  response; `isAccountPath` becomes the generated matcher plus `/cores` plus the `sharedinstances`
  API, which stays routed to the backend for upstream's untouched module (D-90); Amberite edits to
  `users/v3.ts` and `sessions/v2.ts` revert to upstream where no caller in lane A's files needs them
  (the rest are listed for A40); deletes the hand regex and the last `index.ts` fall-through chain.
- Green check: `pnpm --filter @amberite/backend test` and `typecheck`;
  `vp test run scripts/self-hosted-routing.test.ts`; `check:changed`; `vp run up 1 2`, sign out and
  in with a password, enable and disable 2FA, list and revoke a session, change email; a Modrinth
  project page still loads from Modrinth; dev sign-in (`/_dev/session`) still works on a fresh
  scenario.
- Risk: medium. A routing miss sends credentials to Modrinth or account calls to Modrinth.
- Traps: `/auth/login` answers `2fa_required` at HTTP 200 (`accounts/index.ts:372`); any other
  `/auth/*` is 501 (`:536`). `SelfHostedFeature` strips `Authorization` from every non-backend
  labrinth request; keep that. `/_internal/globals` and `/_internal/admin/user/...` aliases are how
  upstream internal modules call. The App's Rust caller of `/_dev/session`
  (`apps/app/src/dev.rs:82`) is lane A's file; it moves to `call::<backend::dev::Session>()` in A37,
  and the path must not change before then.

### B9 refactor(store): the store's five routes are contract Endpoints

- Lane: B-protocol
- Depends on: B8, A10 (MP1)
- Goal: `contracts::store` defines the five store routes; sharing-storage serves them through
  `server::route` and every caller uses `call`/`call_stream`.
- Scope: `packages/contracts/src/{store,hash}.rs`, `packages/contracts/ts/contracts.ts` (generated),
  `packages/sharing-storage/src/{routes,objects,pull,lib}.rs`,
  `packages/sharing-client/src/pool/{store,fetch,mod}.rs`,
  `packages/sharing-client/src/push/upload.rs`, `apps/core/src/{storage,api/typescript}.rs`,
  `packages/engine/src/self_hosted/pool/mod.rs`,
  `packages/api-client/src/modules/copal/generated.ts` (generated). About 18 files.
- Adds / Changes / Deletes: `ListVersion`, `UploadVersion`, `MissingObjects`, `GetObject`,
  `PullVersion`; `StoreHost`; `StoreClient` keeps its pinned-TLS address loop and calls
  `call::<E>()` per address; `apps/core/src/storage.rs:471` uses `call::<PullVersion>`. Deletes
  client `Have`/`UploadOutcome`/`ListedObject` (`pool/store.rs:547-567`), server
  `Have`/`HaveAnswer`/`Uploaded`/`Pulled`/`VersionList`, `PoolCore`, the store's `Hash` newtype.
- Green check:
  `SQLX_OFFLINE=true cargo test -p contracts -p sharing-client -p sharing-storage -p copal -p engine`;
  `check:changed`; `vp run up 1 2`, publish from scenario 1, install the share in scenario 2
  (upload, have, list, ranged object fetch), check the Core's storage page.
- Risk: medium. Streaming and Range paths are easy to get subtly wrong.
- Traps: the tar upload streams `version.json` first (`routes.rs:163`); `object` answers 200/206/416
  on `Range: bytes=N-` (`routes.rs:249`); `list` reads a size-capped body (`MAX_VERSION_LIST_BYTES`,
  `pool/store.rs:313,331`) and is rate-limited by the `X-Forwarded-For` Core's forward sets; the
  store's `/store/{*rest}` forward in Core (`apps/core/src/api/mod.rs:302`) is not an Endpoint.
  `self_hosted/pool/mod.rs` is in the B window: A does not touch it until B10 merges.

### B10 refactor(sharing): shared instance routes are contract Endpoints and upstream's module is upstream again

- Lane: B-protocol
- Depends on: B9
- Goal: `contracts::backend::sharing` (41 routes, Core-key variants folded in,
  `/v1/instances/{id}/servers` included) is the only definition, on upstream's paths and shapes for
  every route upstream calls; sharing-client's `Backend` is gone; Rust callers use `call::<E>()`;
  upstream's `shared-instances` module in api-client is byte-identical to upstream again; jargon
  names are gone (D-90).
- Scope: `packages/contracts/src/backend/sharing.rs`, `packages/contracts/ts/contracts.ts`
  (generated),
  `apps/backend/src/{index,sharing/index,sharing/pool,cores/servers,common/notifications}.ts`,
  `apps/backend/tests/sharing.api.test.mjs`,
  `packages/sharing-client/src/{backend/**,versions/**,pool/**,push/**,install/mod,crypto,lib}.rs`,
  `packages/sharing-client/tests/backend_status.rs`,
  `packages/engine/src/api/instance/shared/{client,mod,history,publish,install,report}.rs`,
  `packages/engine/src/{install/model,install/shared_instance,self_hosted/pool/mod}.rs`,
  `apps/core/src/{api/share,api/content,access/sync,share_pack,api/typescript}.rs`,
  `apps/core/src/state/servers/{commands,domain,adapters}/**` (the sharing files),
  `packages/sharing-storage/src/{objects,local}.rs`,
  `packages/api-client/src/modules/shared-instances/**` (back to upstream),
  `packages/api-client/src/sharing/**` (new),
  `packages/api-client/src/{contracts/labrinth-compat.ts,modules/index.ts,index.ts}`,
  `apps/app-frontend/src/{features/servers/linked-servers.ts,features/settings/sharing/use-shared-storage.ts}`,
  `docs/internals/shared-rust.md`. About 90 files.
- Adds / Changes / Deletes:
  - Sharing Endpoints and types (`InstanceVersion`, `VersionManifest`, `ExternalFile`,
    `NewInstanceVersion`, `InstanceUsers`, `InviteLink`, `Invite`, `VersionDownload`,
    `VersionUpload`, `HostedServer`, `ShareStorage`); `spliceJson`; sharing `parse` functions;
    `labrinth-compat.ts` asserts the generated types fit upstream's `SharedInstances` types.
  - `packages/api-client/src/modules/shared-instances/**` returns to upstream byte for byte and
    keeps reaching the backend through `SelfHostedFeature`'s `sharedinstances` routing. The Amberite
    additions (`players`, `storage`, the `module.ts` base, `errors.ts`) move to
    `packages/api-client/src/sharing/` under the same client keys with bodies over `client.backend`
    and generated types, so every caller, `packages/ui`'s included, compiles unchanged; A40 then
    moves our own callers and deletes what only they used.
  - The engine's sharing functions call contract types and Core-key calls take the acting user
    (D-38); the engine's hand-written sharing types (`api/instance/shared/types.rs`,
    `client.rs:17-93`) stay as the shapes its Tauri commands return, built from the contract types
    inside the engine, because `apps/app/src/api/{instance,install}.rs` name them and are lane A's;
    A34 deletes them with the last commands (the Tauri command layer is the developer's
    module-by-module exception).
  - `ShareVersion` methods become sharing-client free functions; JSON keys `pool`/`store` become
    `download`/`upload`; the R/P/C comments (47 lines) are rewritten; the `Side` re-export left by
    B3 goes once the engine imports `contracts::Side`.
  - Deletes `sharing-client/src/backend/` (21 fns, the `already has access` text match at
    `mod.rs:496`), `versions/model.rs` types, `PoolGrant`, `StoreGrant`,
    `RemoteSharedInstanceUsersResponse::Legacy` and the App's 405 tolerance,
    `ShareVersion.backtrack_version`, the always-404 server-icon route,
    `getServers`/`getServerTicket`, backend `PATCH /v1/storage`, `/access`, `/history` (each after a
    grep confirms no caller), backend local `Instance`/`Version`/`Manifest`/`ManifestFile`.
- Green check:
  `SQLX_OFFLINE=true cargo test -p contracts -p sharing-client -p sharing-storage -p copal -p engine`;
  `pnpm --filter @amberite/backend test` and `typecheck`; `vp test run packages/api-client`;
  `check:changed`;
  `git diff $(git merge-base HEAD modrinth/main):packages/api-client/src/modules/shared-instances HEAD:packages/api-client/src/modules/shared-instances`
  is empty, and `packages/ui` is unchanged by this slice; `vp run up 1 2` on fresh backend data
  (D-109): scenario 1 shares an instance with a mod and a config file and invites scenario 2 by
  link; scenario 2 previews, joins and installs; scenario 1 pushes a second version and scenario 2
  sees the update; the owner removes the member; a Core server follows the share and the players
  list loads; Settings > Sharing storage loads. Merging this is MP2.
- Risk: high. Largest backend file, the splice, four caller sets and the wire key rename land
  together.
- Traps: `versionResponse` splices text (`sharing/index.ts:327`); keep `Cache-Control: no-store`.
  `coreSharing` dispatch happens before any user check (`sharing/index.ts:1494`). The read ticket
  path (`sharing/index.ts:1415`, `apps/core/src/api/content.rs:938`). `delete_share` treats 404 and
  410 as done (`backend/mod.rs:213`); `accept_pending_invite` maps 404 to false (`:502`). The
  backend writes `url: "pool:<object>"` that Rust drops (`versions/model.rs` test). TS
  `modrinth_ids` is optional while Rust requires it: make the contract required.
  `Unavailable::from_status` semantics (404 deleted, 401 revoked) must survive on `CallError`.
  `/v1/instances` does not use `routePath()` (`sharing/index.ts:1470-1476`). Background install jobs
  call sharing outside a request: they use the engine's backend credential (A7). Upstream's module
  paths (`api: 'sharedinstances'`, version 1) fix the backend paths for the routes it calls: change
  our paths, never theirs. Do not touch `apps/app/**`.

### C1 refactor(app-frontend): native calls outside helpers go through one platform seam

- Lane: C-frontend-tooling
- Depends on: F7
- Goal: every non-invoke Tauri call in components goes through
  `apps/app-frontend/src/platform/{os,dialog,opener,window,files}.ts` wrappers that still call Tauri
  with the same signatures; an oxlint rule forbids plugin and window imports outside the seam; the
  M1 to M3 baseline exists. Behaviour does not change.
- Scope: `apps/app-frontend/src/platform/*.ts` (new), about 30 component and provider files that
  import `@tauri-apps/plugin-{dialog,opener,os,fs}`, `api/app` or `api/window` (import lines only),
  `apps/app-frontend/src/main.js`, root `vite.config.ts`,
  `packages/browser-bridge/test/inventory.test.ts` (deleted), `scripts/check-changed.ts`. About 36
  files.
- Adds / Changes / Deletes: adds `platform/os.ts` (`platform`, `type`, `version`, `getVersion`),
  `dialog.ts` (`open`, `save`), `opener.ts` (`openUrl`), `window.ts` (`isNativeWindow`, guarded
  chrome), `files.ts` (`readFile`); changes import lines in the 12 dialog, 11 opener, 6 plugin-os, 4
  `getVersion` and 3 `readFile` files; adds the `no-restricted-imports` rule with an override for
  `platform/**` and the helper files; deletes the browser-bridge inventory test and its
  `check-changed.ts` reference. Records M1 to M3 in the PR (native window with
  `RUST_LOG=webview=info`, and the bridge tab). No parity harness (D-8).
- Green check: `check:changed`; `vp run lint`; `vp run up 1`, open the bridge URL and check Library,
  instance content, settings, a dialog (from the native window with `--show-windows`) and an
  external link. Merging this is MPa.
- Risk: low. It moves imports.
- Traps: plugin-os reads are synchronous at setup time (`appearance-settings.vue:22`,
  `pages/instance/worlds/index.vue:606`): the wrappers stay sync. 44 files are byte-identical to
  upstream: touch only import lines. `packages/ui` has no Tauri imports; its stray
  `__TAURI_INTERNALS__` read (`console/layout.vue:174`) stays. The lint rule must not flag
  `@tauri-apps/api/core` yet (helpers still invoke).

### C2 feat(dev): the dev runner gives each App a fixed local address and prints a plain-tab URL

- Lane: C-frontend-tooling
- Depends on: C1
- Goal: before the App reads them (A4), `vp run up` picks a free port and a secret per scenario,
  passes them as `localPort`/`localSecret` in the dev config, appends `#local=<base>` to the printed
  bridge URL, prints `tab: http://scenario-<n>.localhost:<vite>/#local=<base>` next to it, and keeps
  both in `.data/runtime.json`; `rs <n>` reuses them (D-62, D-64). Landing first means no `up` URL
  is broken between A4 and this change.
- Scope: `scripts/dev-runner.ts`, `scripts/dev-runner.test.ts`, `apps/app-frontend/vite.config.ts`
  (`server.allowedHosts: ['.localhost']`), `docs/internals/scripts.md`, `AGENTS.md` (dev servers
  lines), `.agents/skills/test-app/SKILL.md`. About 8 files.
- Adds / Changes / Deletes: as in Goal; `status` prints the tab URLs; the secret never goes to
  `.data/dev.log`. The App ignores the two dev-config fields until A4 (`DevAppConfig` has no
  `deny_unknown_fields`), and the frontend ignores the fragment until A4.
- Green check: `vp test run scripts/dev-runner.test.ts` (new rows: port and secret stable across
  `rs`, tab URL shape, the printed bridge URL carries `#local=`); `check:changed`; `vp run up 1 2`,
  both bridge URLs still load the App; the tab URLs load Vite (they work from A4 on, and the docs
  say so).
- Risk: low. Dev-only.
- Traps: `dev-runner.ts:1545` `runtimeIsReady` still waits for `bridgeUrl` until C4. Starting from
  Vite 6, `allowedHosts` blocks unknown Host headers. Vite binds `127.0.0.1`; `*.localhost` resolves
  there in Chromium and Firefox. Scenario numbers are unbounded, so pick free ports at spawn instead
  of offsets. The bridge URL comes from the App's log line (`dev-runner.ts:395-398`): append the
  fragment when it is printed and stored, not in the App.

### C3 chore: merge main into the integration branch before the Core lane

- Lane: C-frontend-tooling
- Depends on: A39, B10
- Goal: main's upstream syncs since F7 are in the integration branch before lane D forks.
- Scope: conflict resolutions, usually under 30 files.
- Adds / Changes / Deletes: as F7, on `restructure/72-sync-main`; ported upstream changes to deleted
  Tauri command files are ported into their routes or listed for the owning lane.
- Green check: `git ls-files packages/app-lib` is empty; `check:changed`;
  `SQLX_OFFLINE=true cargo test --workspace --exclude toasty --locked`; `vp run up 1`, the App loads
  in the plain tab. Merge commit. Merging this is MP3.
- Risk: medium. Upstream changes to files the lanes rewrote.
- Traps: as F7. A modify/delete on a deleted command file is upstream behaviour to port into the
  route, not restore.

### C4 feat(dev): agents use the App in a plain browser tab and the browser bridge is gone

- Lane: C-frontend-tooling
- Depends on: C3
- Goal: `vp run up 1 2` prints only tab URLs; readiness is App `/health` plus the Vite module probe;
  the test-app skill drives the tab; `packages/browser-bridge` and every bridge reference are gone;
  the lint rule forbids `@tauri-apps/api/core` outside the seam; M1 to M3 and the hidden dev
  webviews' memory are recorded against C1's baseline. The native window stays config-created and
  hidden dev runs keep `hide_window` (D-65).
- Scope: `packages/browser-bridge/**` (14 files deleted), root `Cargo.toml`,
  `apps/app/{Cargo.toml,package.json,src/main.rs,src/dev.rs,src/cores/proxy.rs,src/server/guard.rs}`,
  `scripts/{dev-runner.ts,dev-runner.test.ts,check-changed.ts}`, `.github/workflows/ci.yml`,
  `apps/app-frontend/src/main.js`, root `vite.config.ts`, `.agents/skills/test-app/SKILL.md`,
  `docs/internals/scripts.md`, `AGENTS.md`. About 20 files plus 14 deleted.
- Adds / Changes / Deletes: the runner's `BRIDGE_READY`, `bridgeUrlFromLine` and `setBridgeUrl`
  become the tab URL built at spawn (`runtime.json` `scenarios[].bridgeUrl` becomes `tabUrl`);
  `appPageLoads` targets the Vite URL; `RUST_LOG` drops `browser_bridge`;
  `--features browser-bridge` and the ci.yml lines go; `proxy.rs:199`'s bridge cfg and the bridge
  origin in the guard's debug allow list go; docs rewritten per the AGENTS.md documentation rules.
- Green check: `check:changed`; `vp test run scripts/dev-runner.test.ts`;
  `cargo test -p theseus_gui`; `vp run up 1 2`, open both printed URLs in the preview: each shows
  its own account, a mod installed in tab 1 appears live, reloading keeps the session;
  `vp run up 1 --show-windows`, the native window works (chrome, window-state restore); run the M1
  to M3 snippet and record it with the hidden webviews' memory.
- Risk: medium. The dev runner's readiness and every agent's entry point change.
- Traps: `dev-runner.test.ts:243,286-293` assert the bridge input. `scenario-state.ts:18,24` match
  the `theseus_gui` process name, which stays. The secret goes in `runtime.json` and `up` output,
  never `.data/dev.log`. If M3 shows connection saturation, move local files to the
  `localhost:<port>` alias host (D-76). Root `Cargo.toml` is C's until this merges; D10 waits for
  it. If lane D's D4 or D5 has merged when this rebases, reset `.data` per D-109 before the check.

### D1 feat(core): the Core starts its own engine and checks access through CoreAccess

- Lane: D-core
- Depends on: C3 (MP3)
- Goal: `CoreState` holds an `Arc<Engine>` on `<root>/engine` with `instances_dir = <root>/servers`
  and `side = Server`; every Core handler checks access through `CoreAccess`, which implements
  `engine::access::Access` and keeps sessions, revalidation and stream caps as its own methods
  (D-81); no engine routes are mounted yet.
- Scope:
  `apps/core/{Cargo.toml,src/daemon/{mod,paths,folders}.rs,src/access/{mod,local}.rs,src/api/*.rs}`,
  `packages/engine/src/state/content_store/store.rs` (lock message). About 45 files.
- Adds / Changes / Deletes: `engine` in `FOLDERS` (`ServersAccess::None`), created on start for
  existing data folders (D-92); `Caller`, `Level::{Viewer,Editor,Owner}` and `Caller::require`
  become `engine::access::{Caller, Permission::{View, Edit, Owner}}` and `CoreAccess::require`,
  which returns the granted level (no role 404, too low 403); `Dangerous` stays Core-only; the
  daemon installs the crypto provider, sets the Core key as the engine's backend credential, and
  builds `Engine::new(EngineConfig { .. })`, starting nothing desktop (D-99).
- Green check: `SQLX_OFFLINE=true cargo test -p copal` (the role-table tests at
  `api/tests.rs:216,677` pass unchanged; a test asserts the Core's engine.db table set equals A38's
  allowed list, D-48); `check:changed`; `vp run down`, delete this worktree's `.data/scenarios` and
  `.data/core` (D-109, primary's copies predate A38), rebuild the scenarios, `vp run up 1`: the Core
  starts and the hosting panel behaves as before.
- Risk: medium. The Core links the engine for the first time.
- Traps: Gradle and `.env` are needed for the Core build (F2 covers CI, release and the dev runner).
  The first Core build with the engine takes about 5 minutes. Install the crypto provider before the
  first engine fetch or it panics. The content store's `store.lock` error says "another Modrinth
  process" (`content_store/store.rs:32-53`): reword it. The 404-for-no-role text is at
  `access/mod.rs:217`. Keep `Caller::Local` as owner without a ticket (`access/mod.rs:299`). Routes
  that read `level` today (`insight.rs:174`, `api/servers.rs:455`) read the `Permission` that
  `require` returns.

### D2 feat(engine): hosts can choose an instance's id and folder

- Lane: D-core
- Depends on: D1
- Goal: engine instance creation accepts an optional id and folder name, so a Core registers a
  server under its id at `<root>/servers/<id>` (D-93).
- Scope: `packages/engine/src/install/runner.rs` create paths,
  `packages/engine/src/state/instances/commands/create_instance.rs`,
  `packages/engine/src/endpoints/instances.rs`. About 6 files.
- Adds / Changes / Deletes: optional `id` and `folder` on `NewInstance`; validation keeps
  `validate_instance_path` (no `..`) and rejects an id already in use.
- Green check: `SQLX_OFFLINE=true cargo test -p engine` (create with a chosen id and folder under a
  custom `instances_dir`); `check:changed`; `vp run up 1`, create an instance in the App as before.
- Risk: low.
- Traps: App ids carry a `local:` prefix, Core ids are bare UUIDs; nothing may parse the prefix.

### D3 refactor(engine): Core's sandbox, archive checks, pack-host guard and server jar inspection live in the engine

- Lane: D-core
- Depends on: D2
- Goal: the folder sandbox, archive checks, pack-host guard (`hosts::client()`) and Paper/Fabric
  server jar inspection exist once, in the engine; the Core imports them; behaviour is unchanged.
- Scope:
  `apps/core/src/state/servers/adapters/{filesystem/folder.rs,filesystem/archive.rs,network/hosts.rs}`,
  `packages/bedrock/src/upstream/jar/{mod,server}.rs`,
  `packages/engine/src/util/{sandbox,archive,pack_hosts}.rs` (new),
  `packages/engine/src/state/instances/commands/embedded_content_metadata.rs` (+`mod server;`),
  `.../embedded_content_metadata/server.rs` (new), importers in
  `apps/core/src/{installs/{forge,mod}.rs,backups/mod.rs,state/servers/**,api/content.rs}`,
  `packages/engine/Cargo.toml`, `upstream.json`. About 30 files.
- Adds / Changes / Deletes: moves the three Core files with their 24 tests and bedrock's
  `upstream/jar/server.rs` with its 5 tests into the engine; every Core importer of
  `bedrock::upstream::jar` (`read_icon`, `Platform`, `Manifest`, `Cache`, `inspect`) switches to the
  engine's copy; bedrock's remaining jar clone (`mod.rs`, `embedded_content_metadata.rs`) stays only
  for `bedrock::content` (`content/domain/sides.rs` imports `upstream::jar::Environment`) and is
  deleted with `content/` in D5 (D-103). Bedrock cannot import the engine, so this is the only order
  that compiles.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p copal -p bedrock`; `check:changed`;
  `git grep -n "bedrock::upstream::jar" apps` prints nothing.
- Risk: medium. `server.rs` was written against the clone's private parsers.
- Traps: the clone differs from the original by 160 diff lines
  (`bedrock/src/upstream/jar/embedded_content_metadata.rs`): check every private parser `server.rs`
  calls by name and signature in the original. `folder.rs` uses `cap-std`/`cap-fs-ext`; add them to
  the engine. `archive.rs` reads `contracts::limits` and `sharing_client::disk`. The `mod server;`
  line is the only edit to the upstream file. Nothing here applies to the App yet; D4 wires these in
  for `side == Server` only (D-95).

### D4 feat(engine): a server-side engine lists, installs and exports server content

- Lane: D-core
- Depends on: D3
- Goal: an engine with `side = Server` lists, installs, updates and exports server content as the
  Core does today: plugins, Paper, world datapacks, the client-only filter, client-only overrides,
  kept or replaced edited files, side review and repair, the sandbox, archive checks and pack-host
  guard. The App (`side = Client`) is unchanged: every hardening step is gated on the side (D-94,
  D-95).
- Scope: `packages/engine/migrations/<init>.sql` (columns added to the fresh schema),
  `packages/engine/src/state/{instance_types.rs,instances/model/{content_entry,file}.rs,instances/adapters/sqlite/content_rows.rs,instances/commands/{list_content,apply_content_install,apply_content_update,content_mutation,sync_content_files,create_instance}.rs,instances/watcher.rs}`,
  `packages/engine/src/api/{pack/install_mrpack.rs,instance/export_mrpack.rs,instance/files.rs,instance/shared/install.rs}`,
  `packages/engine/src/util/fetch.rs` (the server download client),
  `packages/engine/src/state/mod.rs`, `packages/engine/src/{routes,endpoints}/**`,
  `packages/sharing-client/src/rules/**`, `packages/bedrock/src/**` (exhaustive matches only),
  `packages/engine/.sqlx/**` (generated). About 42 files.
- Adds / Changes / Deletes: `ModLoader::Paper`, `ProjectType::Plugin`; columns `sides_unlocked`,
  `server_override`, `pending`; the server DataPack folder from `server.properties` `level-name`;
  the install and update filter uses the content's server requirement, keeps edited files as
  `pending` and honours `server_override`; export writes `server-overrides/` and
  `client-overrides/`; Endpoints `SetSide`, `SetSidesUnlocked`, `CompleteSideReview` (Core's
  `addons/complete-side-type-review`), `RepairContent` (Core's `content/repair`, over the engine's
  existing repair), `EditedFiles`, `DecideEdited`, `PlaceClientOnly`; watched subpaths depend on the
  side. A server-side engine holds `hosts::client()` (D-35, D-95) and downloads every pack file,
  override and share file through it; archive extraction and the folder sandbox run only on that
  side. Tests ported from `domain/pack.rs` (15), the export overrides (2) and `hosts.rs`, plus a new
  one: a pack URL that redirects to a private address is refused.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p sharing-client -p bedrock`;
  `check:changed`; `vp run down`, delete this worktree's `.data/scenarios` and `.data/core/engine`
  (the edited init migration fails sqlx's checksum on every existing engine.db, D-109), rebuild the
  scenarios, `vp run up 1`, install an .mrpack into a desktop instance and the content tab is
  unchanged; an .mrpack with a symlinked `mods/` still installs on desktop.
- Risk: high. Behaviour changes in upstream install code.
- Traps: `ModLoader::from_string` falls through to Vanilla (`instance_types.rs:224-231`). Every
  exhaustive `match` on `ProjectType` and `ModLoader` in the engine, in bedrock (it compiles
  `instance_types.rs` by `#[path]`) and the generated TS unions gains a variant. The watcher creates
  missing subfolders (`watcher.rs:457-470`): do not create `resourcepacks/` and `shaderpacks/` in
  server folders. Client-side mrpack skipping is at `install_mrpack.rs:303,790`; mirror it, do not
  fork it. Rules keep plugins out of shares (`domain/server_share.rs`). Pending semantics:
  `apps/core/migrations/001_init.sql:301-308` and `state/servers/commands/edited_files.rs`. Desktop
  users symlink `mods/` and keep less than 256 MiB free on small disks: the sandbox, link refusal
  and `DISK_MARGIN_BYTES` never run on the client side. Tell C4 (open in parallel) to reset `.data`
  the same way if it rebases onto this.

### D5 feat(core): a Core's content and sharing run on the engine, and the duplicate half is gone

- Lane: D-core
- Depends on: D4
- Goal: a Core server's content, modpack, share push and follow, history and update channel go
  through the engine's routes behind `CoreAccess`; the Core's duplicate content and sharing half is
  gone, and with it bedrock's `content/` and the jar and store clones; core.db is fresh; the hosting
  panel's content and share screens work through the api-client adapter.
- Scope: deletes `apps/core/src/state/servers/**` except `adapters/filesystem/properties.rs`,
  `adapters/sqlite/server_rows.rs`, `commands/{apply_pending,apply_pending_on_stop,forget_pack}.rs`
  and `model/properties.rs` (moved to `apps/core/src/servers/`); deletes
  `apps/core/src/api/{content,share}.rs`, the content tests in `api/tests.rs`,
  `apps/core/src/share_pack.rs`, `packages/sharing-client/src/consume/**` remnants,
  `packages/bedrock/src/{content/**,upstream/jar/**,upstream/store/**}` and their `lib.rs`
  re-exports (D-103); rewrites `apps/core/migrations/001_init.sql` and `apps/core/src/db/mod.rs`
  (`core.db`); touches
  `apps/core/src/{daemon/mod.rs,servers/{mod,launch,watch,memory,restarts}.rs,installs/mod.rs,network/forward.rs,api/{mod,servers,typescript}.rs,api/engine_mount.rs}`,
  `apps/core/package.json` (the `types` script), `packages/engine/src/endpoints/**` (share, diff,
  history, edited-file types),
  `packages/api-client/src/core-hosting/{index,content,kyros-content}.ts` (new),
  `packages/api-client/src/modules/{archon,kyros}/content/v1.ts` (back to upstream),
  `packages/api-client/src/modules/{index.ts,copal/**}` (deleted),
  `packages/api-client/src/index.ts` (the `Copal` type names `packages/ui` imports, as re-exports of
  generated types), `.prettierignore`. About 145 files, mostly deletions.
- Adds / Changes / Deletes: mounts the engine's content and share routes on the Core router; the
  hooked ones are mounted by wrapping the engine handler in `api/engine_mount.rs` (D-84):
  `with_backup` on modpack update, loader change and share restore, `with_log` on every mutation,
  `with_forget_players` on unlink; creating a server creates the engine instance with the server id,
  deleting one removes it; the Core reads game version and loader from the engine; the Core calls
  `mark_running` from its lifecycle and takes the engine lock to start; `pending_packs` applies via
  the engine on stop (`apply_pending_on_stop` stays a Core step, D-94); a server-icon writer listens
  to engine events (D-101); the engine's history and retention are used and `bedrock/tests/store.rs`
  is ported to an engine content-store test; share calls pass the caller's user id as the acting
  user (D-38); the share-history `SCHEMA` and the bedrock store schema stop running on core.db
  (`db/mod.rs:49,53`); the last share and diff types are engine Endpoint types and `typescript.rs`,
  `copal/generated.ts` and `apps/core/package.json`'s `types` script (it runs the deleted
  `write_typescript`) are deleted; `withCoreHosting` and about 31 adapter methods, with
  `restoreClientFiles`, `getUpdateGameVersionPreview` and `applyGameVersionUpdate` answering
  `ApiError { error: unsupported }` (D-102); Core's owed share-deletion retry goes (D-100).
- Green check:
  `SQLX_OFFLINE=true cargo test -p copal -p engine -p sharing-client -p contracts -p bedrock`;
  `vp test run packages/api-client/src/core-hosting` (one test per hook wrapper: a pack update
  through the mounted route makes a backup first); `check:changed`;
  `git diff origin/refactor/restructure-repo -- packages/ui` is empty for this slice; `vp run down`,
  delete this worktree's `.data/core` (D-109), re-pair the dev Core, `vp run up 1 2`; in tab 1:
  create a server, install a Fabric modpack (a client-only mod is skipped and can be placed on
  request), disable, enable and update a mod, start the server (content edits refuse while it runs),
  edit a config, update the pack (the edited file shows keep or replace, a backup appears first),
  share the server and follow it from tab 2, restore a history version, unshare.
- Risk: high. The largest behaviour surface; every server-side content rule is re-expressed.
- Traps: `servers.hold` refusals (`servers/mod.rs:839-856`) must map to the same `ApiError` codes;
  the UI matches on `Installing` and `AlreadyRunning`. `content.rs:118` shares one broadcast with
  backups; keep backup progress working until D8. `adapters/sqlite/content_rows.rs` writes
  `OwedCall::ForgetPlayers`: find each trigger before deleting it. `write_shared_pack.rs` uses the
  Core key for the caller's user. `apps/core/src/api/tests.rs:1632,2093` import bedrock. Keep the 30
  s timeout and 10 MB cap layering from `api/mod.rs:227,285` for the mounted engine routes. Do not
  mount path-taking engine routes on the Core (D-52). The adapter methods are typed
  `Partial<Pick<UpstreamModule, ...>>` so signatures stay upstream's. bedrock's
  `content/domain/sides.rs` is the last user of its jar clone: delete them together.

### D6 feat(engine): the engine installs servers and builds their Java command

- Lane: D-core
- Depends on: D5
- Goal: on a server-side engine, install downloads the server jar (no assets or natives) and runs
  the loader's server install; `server_command(engine, id)` returns the Java path, arguments and
  working directory; Core's supervisor and shim run it; `apps/core/src/installs/**` is gone (D-96).
- Scope: `packages/engine/src/launcher/{download.rs,mod.rs,server.rs}`,
  `packages/engine/src/install/**`, `packages/engine/src/api/jre.rs`, `apps/core/src/installs/**`
  (ported then deleted), `apps/core/src/servers/{mod,launch}.rs`,
  `apps/core/src/api/{servers,startup}.rs`, `apps/core/src/cli/**` (Java and install commands).
  About 35 files.
- Adds / Changes / Deletes: the server download set; processor rules with `SIDE=server`
  (`launcher/mod.rs:614-662`); Paper from Core's `paper.rs`; `ServerCommand { java, args, cwd }`;
  Java selection through the engine's jre module; deletes Core's installers and its separate Java
  installer. Tests: processor selection by side; arguments for vanilla and Fabric match Core's
  current `apps/core/src/installs/shared/launch.rs` output.
- Green check: `SQLX_OFFLINE=true cargo test -p engine -p copal`; `check:changed`; `vp run up 1`, in
  the tab create a vanilla server and a Fabric server, start both, the console shows the server
  ready; change memory and see it in the process arguments; install a Fabric modpack in a desktop
  instance and launch it (client path unchanged).
- Risk: high. Loader server installs differ per loader, and server start changes path.
- Traps: `apps/core/src/installs/{forge,paper,java}.rs` hold the working server logic: port, do not
  rewrite. The shim runs as `copal-servers`; the engine only builds the command
  (`apps/core/src/daemon/paths.rs:29` ACL model). Java downloads land in the engine's folder
  (`ServersAccess::None`), so the shim must still be able to execute them: check the ACL on the Java
  folder.

### D7 feat(core): files, icons and download links run on the engine

- Lane: D-core
- Depends on: D6
- Goal: file manager, icon and export operations are engine Endpoints behind the sandbox; every
  download uses one `POST /links` token; the Kyros file routes, the `/fs` handshake and all
  authorize routes are gone (D-97).
- Scope: `apps/core/src/api/{files,icon,export,backups}.rs`, `apps/core/src/access/links.rs` (new),
  `apps/core/src/backups/mod.rs` (grant and take removed),
  `packages/engine/src/{api/instance/files.rs,routes/files.rs}`, `packages/engine/src/endpoints/**`,
  `packages/contracts/src/core/**`,
  `packages/api-client/src/core-hosting/{files,upload-sessions,icons,kyros-backups,logs}.ts`,
  `packages/api-client/src/modules/kyros/{files,upload-sessions,backups,logs}/**`,
  `packages/api-client/src/modules/archon/icons/v1.ts` (back to upstream),
  `packages/api-client/src/{core/abstract-upload-client.ts,platform/tauri.ts}` (Core branches only),
  `apps/app-frontend/src/platform/adapters/file-download.ts`. About 40 files.
- Adds / Changes / Deletes: moves Core's stat, zip, unarchive, ops and upload-session handlers into
  the engine; engine icon routes replace `icon.rs`; `CoreAccess::caller` redeems `?link=` only for
  its exact GET path; about 28 adapter methods; the files half of the role-table test moves to the
  engine.
- Green check: `SQLX_OFFLINE=true cargo test -p copal -p engine`; `check:changed`; `vp run up 1 2`:
  in a Core server's Files tab list, open and edit `server.properties`, upload a folder, unarchive a
  zip, zip a folder, download a file and the world, export an mrpack, set an icon; scenario 2 as
  Viewer gets 403 on the file list and a stranger gets 404.
- Risk: high. Security-sensitive.
- Traps: `folder.rs:38` `clean()` is the path canonicaliser; every server-side engine file handler
  must go through it. Link grants are bound to the exact path, one-use, and redeem re-runs `require`
  (`api/backups.rs:801-822`). Uploads have no body cap (`api/mod.rs:285`). `overview.vue:96` reads
  `/logs/latest.log` through `kyros.files_v0.downloadFile`; the adapter must serve it. The zip route
  answers `application/json-seq` progress today (`files.rs:1100-1108`).

### D8 feat(core): one event socket with topics replaces the console socket

- Lane: D-core
- Depends on: D7
- Goal: the Core serves its own socket loop at `/events` (D-78) carrying engine events for the
  instances a caller may see and the Core's `CoreEvent` (console, power, backups); revocation
  re-checks per subscription; the Archon WS and the per-server socket are gone.
- Scope:
  `apps/core/src/{event.rs,api/events.rs,api/mod.rs,servers/console.rs,backups/mod.rs,cli/client.rs,access/mod.rs}`,
  `packages/contracts/src/core/**` (`Topic`, `ClientMessage`), `packages/contracts/ts/contracts.ts`
  (generated), `packages/api-client/src/core-hosting/sockets.ts`,
  `packages/api-client/src/modules/archon/sockets/**` (back to upstream). About 20 files.
- Adds / Changes / Deletes: the Core's loop in `api/events.rs`: `Subscribe`/`Unsubscribe` for
  `Topic { Instance, Installs, Console, Server, Backups }`, each checked with `CoreAccess::require`
  (Console needs Edit); on access changes, on lag and every 600 s it re-checks, sends `Ended` for a
  subscription that lost its permission and closes the socket when the session fails; on `Lagged` it
  sends `Resync`; `CoreAccess` keeps `revalidate`, `changes` and `open_stream` (4 sockets per
  session) as its own methods (D-81). One console follower per server fanned out, with a 200-line
  backlog that leaves out health checks; `Power` events replace the 500 ms poll per socket; deletes
  `event.rs`, `/servers/{id}/ws` and `/modrinth/v0/servers/{id}/ws`; the adapter maps topics to
  Archon WS events and `send('command')` to `POST /servers/{id}/console`.
- Green check: `SQLX_OFFLINE=true cargo test -p copal` (revoking a member's role ends their console
  subscription and keeps the socket; ending their session closes it; a lagged subscriber gets
  `Resync`); `check:changed`; `vp run up 1 2`: the console streams and accepts commands; tab 2 as
  Viewer sees power and install progress but no console; revoking tab 2's access in tab 1 ends its
  stream within a second.
- Risk: high. Revocation semantics and backpressure.
- Traps: subscribe to `access.changed` before the role check, as today (`api/events.rs:51`), or a
  revoke in between is missed. Idle expiry uses `sessions::IDLE_S` and `RECHECK_EVERY` of 600 s
  (`api/events.rs:69,165-196`). The broadcast cap is 256 (`backups/mod.rs:157`); on lag, re-check
  and resync, never panic. Watch: if console bursts delay other events, give each subscription a
  bounded buffer that drops old console lines. `cli/client.rs` follows the console too. Each console
  follower is an OS thread polling the file today (`servers/console.rs:107`). The App's loop is
  separate and simpler; share a fan-out helper only if both turn out to need it.

### D9 feat(core): Core server routes drop their Archon shapes

- Lane: D-core
- Depends on: D8
- Goal: servers list, get, create, delete, power, reinstall, startup, properties and backups are
  `contracts::core` Endpoints under `/servers/{id}`; the adapter answers every remaining archon call
  for a Core; every upstream archon and kyros module is byte-identical to upstream again; `Conflict`
  is split into its own codes, including `eula_not_accepted`.
- Scope: `apps/core/src/api/{servers,startup,properties,backups,performance,mod,tests}.rs`,
  `apps/core/src/cli/{servers,backups}.rs`, `packages/contracts/src/core/**`,
  `packages/api-client/src/core-hosting/{servers-v0,servers-v1,backups-queue,backups,options,properties,server-users,actions}.ts`,
  `packages/api-client/src/modules/archon/{servers,backups-queue,backups,options,properties,server-users,actions,types}.ts`
  (back to upstream; A40 already removed the `Cores` namespace from `types.ts`),
  `packages/api-client/src/features/core-session.ts`. About 45 files.
- Adds / Changes / Deletes: reshaped Endpoints with no world id; `permissions(level)`
  (`api/servers.rs:455`) moves into the adapter; about 51 adapter methods; the CLI's server and
  backup commands use the generic caller; the adapter maps `eula_not_accepted` to the shape the
  upstream panel shows; deletes every `/modrinth/v0/*`, `/v1/servers/*`, `/servers/{id}/v1/*` and
  `/local/servers` route and the dead `setLinkedInstance` (D-101).
- Green check: `SQLX_OFFLINE=true cargo test -p copal`; `check:changed`;
  `git diff $(git merge-base HEAD modrinth/main) HEAD -- packages/api-client/src/modules/archon packages/api-client/src/modules/kyros`
  is empty (the last synced upstream commit, not the moving `modrinth/main`); `vp run up 1 2`: walk
  the hosting panel's General, Installation, Properties, Startup, Backups (create, restore with a
  ticket, schedule) and Access pages, power and reinstall; start a server without accepting the EULA
  and see the EULA prompt; `copal servers list|start|stop` work.
- Risk: medium. Mostly reshaping, across every server page.
- Traps: deleting a server and restoring or deleting backups are `Dangerous` (`access/mod.rs:269`);
  keep the extractor. Keeping fewer backups is conditionally dangerous (`backups.rs:544`), and a
  crash's `last_lines` needs Edit inside a Viewer route (`insight.rs:174`).
  `cli/servers.rs:147,502,578,587,596` and `cli/backups.rs:215` hard-code paths.
  `ErrorCode::Conflict` covered EULA, unmanaged, running and no-ports (`api/servers.rs:425`).

### D10 chore(engine): bedrock is gone

- Lane: D-core
- Depends on: D9, C4
- Goal: `packages/bedrock` does not exist; the engine owns `instance_types.rs` and the six model
  files again, as upstream does; `shared-rust.md` is gone.
- Scope: `packages/bedrock/**` (deleted, about 64 files, 22 `.sqlx` generated),
  `packages/engine/{Cargo.toml,src/state/mod.rs:18,src/state/instances/model/mod.rs:8,src/util/mod.rs}`,
  root `Cargo.toml` (member, dependency, `postcard-bindgen`), `apps/{app,core}/Cargo.toml`,
  `packages/sharing-{client,storage}/Cargo.toml`, `docs/internals/shared-rust.md` (deleted),
  `.prettierignore`, `upstream.json`. About 15 non-deleted files.
- Adds / Changes / Deletes: restores the six `mod x; pub use x::*;` lines and `mod instance_types`;
  drops bedrock's `#[path]` borrowing; deletes the crate, `ports.toml`, `build.rs`, `.sqlx`, its
  `export-ts` wiring and the `postcard-bindgen` workspace dependency; `.prettierignore` drops the
  old migrations line.
- Green check:
  `SQLX_OFFLINE=true cargo test -p engine -p copal -p sharing-client -p sharing-storage`;
  `cargo check -p theseus_gui`; `check:changed`;
  `git diff $(git merge-base HEAD modrinth/main):packages/app-lib/src/state/instances/model HEAD:packages/engine/src/state/instances/model`
  shows only our additive changes (upstream keeps the old path, so compare the mapped paths at the
  last synced commit); `git grep -nE "bedrock::|packages/bedrock|bedrock =" -- ':!.plans'` prints
  nothing (a plain `bedrock` grep also matches a Minecraft store URL in an upstream file).
- Risk: low. Module wiring and deletion.
- Traps: `util/content_hash.rs` is compiled twice today; drop only bedrock's `#[path]`. Whatever
  `shared-rust.md` said that is still true goes to Z2's architecture.md (note it in the PR). Root
  `Cargo.toml` is free only after C4 merges.

### Z1 chore: merge main into the integration branch for the final pass

- Lane: final
- Depends on: D10, C4
- Goal: the integration branch has main's latest upstream syncs before the final docs and pass.
- Scope: conflict resolutions, usually under 30 files.
- Adds / Changes / Deletes: as F7, on `restructure/95-sync-main`.
- Green check: `git ls-files packages/app-lib` is empty; `check:changed`;
  `SQLX_OFFLINE=true cargo test --workspace --exclude toasty --locked`; `vp run up 1`, the App loads
  in the plain tab. Merge commit.
- Risk: medium.
- Traps: as F7 and C3.

### Z2 docs: real docs replace the restructure plan

- Lane: final
- Depends on: Z1
- Goal: `docs/internals/architecture.md` records the design as built; AGENTS.md, scripts.md,
  upstream.md and the skills match the code; the temporary plan and the setup fallback are gone.
- Scope: `docs/internals/architecture.md` (new), `.plans/restructure{,-decisions,-slices}.md`
  (deleted), `AGENTS.md`, `docs/internals/{scripts,upstream}.md`,
  `.agents/skills/test-app/SKILL.md`, `.claude/skills/upstream-sync/SKILL.md`,
  `scripts/setup-worktree.ts`, our files still saying "app-lib" in prose (about 15). About 27 files.
- Adds / Changes / Deletes: architecture.md from the doc's lasting parts (Rules, crates and import
  direction, Engine without a global, hosts with their state and databases, Endpoint, ApiError and
  the protocol version, where Endpoints are declared (contracts for types we own, beside their types
  for the engine and App, D-16), who calls whom, the Core proxy and why it keeps its own origin,
  Access, events and resync, browser tabs, Rejected, Watch, the Revisit outcomes the developer
  settled, and a short glossary of pool, store, content store, holdings and "self-hosted" as our
  account backend, D-113) with a link to the sqlx comment in `packages/engine/.cargo/config.toml`;
  AGENTS.md: the import direction in one line linking architecture.md, the labrinth pointer says
  "read at `modrinth/main`", the restructure-only `check:changed` note goes (the One contract bullet
  changed in F5, the test data section in A38); removes the `packages/app-lib/.env` fallback;
  rewords prose "app-lib" in our files.
- Green check: `check:changed`;
  `git grep -n "bridge URL\|browser bridge\|shared-rust\|restructure.md\|restructure-slices\|bedrock::\|packages/bedrock" -- AGENTS.md docs .agents .claude`
  prints nothing; every path and command named in AGENTS.md, scripts.md and upstream.md exists (run
  or `ls` each).
- Risk: low.
- Traps: documentation rules: decisions, cross-component constraints and traps only; do not
  enumerate Endpoints or narrate flows. The sqlx comment and the `ported` rule must survive.
  Upstream skills under `.agents/skills` that reference missing paths are reported, not fixed.

### Z3 test: integrated pass, rebuilt dataset and the final pull request

- Lane: final
- Depends on: Z2
- Goal: the whole restructure is verified in the real App, the shared-instance dataset is rebuilt on
  the new schemas, and one PR to main is open with green CI.
- Scope: no source changes expected; a fix found goes to a `restructure/9N-fix-*` slice first. The
  dataset lives outside the repo.
- Adds / Changes / Deletes:
  - Plain-tab pass from `vp run up 1 2` with the test-app skill, scenario 1 unless noted: Library
    (instances, groups, hidden instances in Settings); Content tab (list, add from Modrinth, toggle,
    update, remove) with progress; Files tab (open, edit, save); Worlds; Logs; instance settings and
    App settings; Browse and a project page; create an instance from a modpack; Play (or the
    Minecraft-required prompt); Skins; Friends between 1 and 2 with presence while 1 plays.
  - Sharing: scenario 1 publishes, scenario 2 joins; scenario 1 publishes an update and scenario 2
    applies the diff; stop the stack, `vp run up 2` only, scenario 2 installs a fresh copy while App
    1 is closed.
  - Core: pair the dev Core with scenario 1 (pairing code in `.data/dev.log`), create a server,
    install content, start it (EULA prompt from the `ApiError` code), console, files, backups,
    properties; invite scenario 2 as Viewer (sees the server, no console), promote to Editor
    (console appears), remove (the open console ends); a Core reporting an older protocol shows the
    update prompt.
  - Native pass with the developer's go-ahead: `vp run up 1 --show-windows`; window chrome, a dialog
    started from a tab opens on the desktop, drag a jar onto the Content tab.
  - Dataset: follow scripts.md "Regenerating the sharing dataset" plus the paired Core with one
    server shared to scenario 2; `vp run down`; copy `.data/{backend,core,scenarios}` with
    `-wal`/`-shm` to a scratch folder outside the repo. Do not touch the primary checkout.
  - Final PR `refactor: one engine, one protocol` from `refactor/restructure-repo` to `main` (real,
    not draft; merge commit): problem, solution, slice PR list, the Revisit list from decisions.md,
    the report list (T-A23, the two `DO-NOT-READ-stale-*` docs, the unused `AdsConsentRequired`
    variant and the engine functions behind dead commands, D-37), the dataset path, model and
    harness.
- Green check: CI green on the final PR;
  `SQLX_OFFLINE=true cargo test --workspace --exclude toasty --locked`; each screen above shows no
  `[crash]` lines in the console; `.data/dev.log` shows no panics.
- Risk: medium. First time every lane's work runs together.
- Traps: the old `.data` copied from primary is unusable (developer decision 3): start from an empty
  `.data` in this worktree. A tab cannot show chrome, dialogs or real-path drops. Data flows only
  into a worktree; the primary's `.data` is replaced by the developer after the merge. The console
  Viewer check is the security-relevant one (`apps/core/src/api/events.rs:165-196` behaviour).
  CodeRabbit will exceed 300 files on the final PR; the slices were the review.
