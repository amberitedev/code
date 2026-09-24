# Account and sharing separation: implementation handoff

Status: ready for Ilai to review and hand to an implementation agent. Writing this plan does not
start implementation. Date: 2026-09-23.

## Outcome

Amberite runs the existing Modrinth account/social and private instance-sharing experience against
our own backend in local development. Public content still comes from Modrinth. Neither test user
needs a Minecraft server or a paired Core to share an instance.

Primary workflow: A adds B as a friend, A shares an existing instance, B accepts and installs it,
A changes content/configs and pushes an update, B reviews and installs that update.

Additional accepted workflows: correct account/session behavior across switching and restart;
recipient installation/update while the owner app is closed, including interrupted transfer retry.

Read [planning-memory.md](planning-memory.md) for decisions, reasons, supersessions, and later ideas.
This file bounds the implementation milestone. The memory is broader and must not become a task list.

## Scope

### Included

- Reproduce existing Modrinth account behavior before adding Minecraft-first changes. Inventory all
  account/social operations exposed by the retained app; do not silently implement only a login stub.
- Reuse/port relevant public Labrinth code and contracts, preserving attribution and licenses.
- Implement the private sharing service behavior consumed by the app using public client code and,
  where available, authorized reference observations from the official installed Modrinth app.
- Real durable account, friendship, invitation, membership, version, and storage-location data.
- Real local storage service, usable without Minecraft process management. Keep the separation small.
- Typed API-client routing/orchestration with minimal necessary native transport integration.
- Preserve existing screens and workflows. Fix integration, not product design.
- Reproducible local development with isolated users, application state, backend state and storage.
- Focused contract, security, restart, and end-to-end verification.

### Excluded

- Minecraft hosting integration, pairing UX, hosted Worlds, unified server/content UI.
- Production deployment, subscriptions, provider account creation, public endpoints, tunnels/relays,
  router or firewall setup, NAS installation, or a second-PC setup.
- Private-Core feature, opt-out incentives, donation/capacity prompts, contributor onboarding.
- Advanced placement prediction, Jev, erasure coding, fleet rollout, broad telemetry collection.
- PRs/branches for shared instances, new collaboration roles, new history/pinning UI.
- New KubeJS/FancyMenu/all-content-type support beyond inherited sharing behavior.
- Direct owner-to-recipient fallback when hosted storage is full.
- Wholesale Core refactor, app reset, deleting Convex before its replacements are verified.

## Architecture baseline and short decision gate

Use one local hosted-backend application with separate account/social and sharing modules. Prefer a
single relational database initially. Keep the storage process separate. One service can preserve
multiple API route namespaces. There is no need for one deployment per upstream service name.

Cloudflare Workers + D1 is the current hosting recommendation, not a previously finalized stack
decision. Before large-scale porting, inventory actual account dependencies and compare adaptation
effort against retaining Rust/Postgres on Cloud Run. State a concrete recommendation to Ilai and obtain
confirmation if this choice remains unapproved. Do not reopen all product decisions or spend days
building competing prototypes. Preserve portable contracts regardless of host choice.

The default implementation should not call a remote account service for every sharing request.
Authenticate centrally and use the same user records. Never send account session secrets to arbitrary
storage contributors; use scoped transfer authorization. Preserve error and revocation behavior.

Dev must use the selected production-compatible backend runtime locally, rather than a fake backend
that passes demos but would need replacement. If Workers is selected, use local Workers/database
tooling. Do not deploy cloud functions to make the dev tests pass.

## Phase 0: inspect and establish evidence

1. Read repository/ancestor AGENTS and applicable skills. Inspect branch, worktrees, dirty changes,
   current launch commands, API-client adapters, native requests, Labrinth, and existing account code.
2. Preserve unrelated changes. Work in an isolated worktree. Do not change main, reset the app, install
   official software, alter a live Modrinth profile, or create/push a PR as part of this handoff.
3. Produce a compact contract inventory: operation, source caller, request/response types, auth,
   persistence, errors, existing implementation, proposed replacement, evidence confidence, test.
4. Reconcile the user's reported Labrinth account port with actual code. Do not implement against an
   assumed old branch or silently preserve an obsolete custom account design.
5. Inventory login providers, credentials, callbacks, email flows, session/revocation/account switching,
   profiles and social behavior. List external dependencies separately from portable source code.
   Missing provider credentials may block verification of a flow; never report a dev bypass as proof.
6. Resolve the runtime gate above. Then assign file ownership and proceed with the agreed design.

### Official Modrinth reference checks

Ilai says access to the official sharing backend requires credentials supplied through the official
installer/application. Treat the installer-issued credential mechanism as user-reported until observed;
do not assume a directory override or source build acquires those credentials.

Use official installed Modrinth for any live reference workflow. The user accepted testing on one PC,
potentially with two official installations/accounts; installation isolation remains to be verified.
Do not promise two simultaneous windows: inspected upstream enables a single-instance plugin.
`THESEUS_CONFIG_DIR` exists in source but is not an established solution for credentials or WebView state.

Try sequential owner and recipient operations with properly isolated official app state if supported.
If setup requires user action, request that bounded action. Do not make another computer a prerequisite.
Do not extract/reuse embedded service credentials outside their intended application, bypass service
access controls, or use third-party accounts. Reference captures must redact tokens/cookies/secrets.
Ask before computer use or opening browser surfaces as required by repository instructions.

Source review can proceed without official access. Read response parsers and UI consumers, not just
requests. Mark uncertain semantics. Use a few reference checks to resolve them; never count mock
responses as evidence of the official backend. Avoid capturing unrelated traffic or private files.

## Phase 1: local environment and contracts

- Reuse the existing dev runner/scenario conventions. Read `docs/internals/scripts.md` first.
- The current primary-checkout dev command can push to cloud Convex. Do not start it blindly.
  Use an isolated worktree and ensure all backend targets are local before running the full stack.
- Keep `.data/` state isolated, persistent across restarts, and separate from the user's official app.
- Prepare A, B, and an unauthorized test user C. Test fixtures must not depend on random live accounts.
- Seed realistic test content: a known public Modrinth mod, a selected supported config, and a harmless
  custom external file of a type already supported by the client. Do not use private user files.
- Use two local storage process instances with separate roots when verifying replica/failover behavior.
  They represent storage endpoints, not Minecraft servers and not simulated independent failure domains.
- Document launch/reset/stop commands and actual ports. Record PIDs; stop only processes started here.
- Keep dev-only account fixtures explicit and unavailable in production configuration.

## Phase 2: accounts and social

- Port/reuse account behavior and social contracts identified in the inventory. Do not recreate
  public project hosting. Do not use Modrinth production user/session data as our own database.
- Implement persistent users, correct identity/session mapping, account switching, friendship and
  associated state/notifications as required by the inherited app.
- Retain upstream account functionality; track every inventoried flow as implemented, inherited,
  externally blocked, or deliberately deferred with user approval. Three acceptance tests alone do
  not establish complete account parity.
- Authentication and authorization stay enforced in backend/storage handlers, not only in the UI.
- Remove old runtime dependencies only after affected operations work against their replacement.

## Phase 3: real sharing and storage

- Implement creation, access/membership, invitations and their inverse operations, version metadata,
  selected configs, external uploads/downloads, readiness, and update retrieval required by the client.
- Use existing public content IDs/download sources. Do not upload all installed Modrinth mod JARs.
- Keep upload/download bytes out of the metadata database. Store durable objects in the storage service.
- A minimal authenticated streaming upload adapter is acceptable if it preserves the API cheaply;
  direct scoped uploads are also acceptable. This is real storage, not a proxy to Modrinth's private API.
- Enforce path safety, bounded bodies/archive expansion, safe temporary writes, verified content hashes,
  and atomic completion. An incomplete upload must never become a successfully installable version.
- Retries must not duplicate accepted changes or corrupt already complete files. Define conflict handling
  from the client contract. Surface meaningful errors rather than silently overwriting version state.
- Two replicas are the initial storage target. Keep placement deterministic/simple and repair bounded.
  Downloads can proceed after one complete verified copy exists while the second is created.
- Encrypt contributed content with an established construction. Keep key access scoped to authorized
  users and recovery; never expose plaintext keys to unrelated storage hosts. Specify recovery and
  revocation semantics before claiming encrypted sharing complete. No novel cryptographic protocol.
- Preserve a local owner recovery snapshot and enough metadata to restore lost remote bytes. Do not
  mistake the mutable working instance for a historical snapshot. Automatic restore can be exercised
  locally; no fleet-wide prediction or contributor setup UI is needed.
- Five recent online versions plus pinned versions is the agreed eventual retention policy. Do not
  build new pin/history screens. Keep storage/history representation compatible with that policy;
  do not delete data through unverified cleanup while stabilizing the workflow.
- Reuse unchanged content where safe. Advanced compression/chunking can wait; do not change upstream
  payload semantics merely to make a theoretical storage saving.

## Phase 4: integrate without redesign

- Route app-facing communication through the existing typed API-client architecture where applicable.
- Native Rust sharing already makes requests. Change only required native transport/auth/configuration
  boundaries. `packages/app-lib` has a repository restriction: obtain scoped permission before editing
  if the implementation task has not explicitly authorized those required files.
- Preserve Modrinth user interactions, error states, invite review, update review and personal-content
  behavior. Any proposed UX departure goes back to Ilai before implementation.
- Keep account sessions separate between A and B. Do not let cached data or notification state leak
  across account switches.

## Verification and evidence

### A. Friend -> share -> accept -> update

Use two real dev app scenarios with real local APIs and storage. Complete friendship, share an existing
instance, accept/install as B, change a public mod reference and a selected config as A, push, then
review/install as B. Verify file hashes and selected versions, not only a success toast. Check the
custom-file path too. Restart backend/storage and confirm persistent state is intact.

### B. Accounts and sessions

Switch accounts and restart the app. Each account sees its own friends, invitations and shares.
Sign out and verify protected actions fail without a valid session. Test expiration/revocation as
supported by the contract. User C must not read private version data, upload, or download shared bytes.

### C. Owner offline and interrupted transfer

Finish an upload, close A's app, and install/update as B. Interrupt B's transfer and retry; verify exact
final bytes and no partially installed successful state. If resumable transfer is not inherited, a
correct bounded restart is acceptable. Stop one storage endpoint and verify the available replica is
used. Separately verify owner recovery from a retained local snapshot if all remote test copies vanish.
Destructive failure tests operate only on clearly designated disposable test data.

### Focused checks and reporting

- Add focused stable tests for contracts, authorization, readiness/integrity, retry and persistence.
- Run targeted tests/lint/typechecks for changed workspaces. No repo-wide checks unless asked.
- One primary-agent integrated app pass after changes are combined; subagents do not start competing
  dev servers. Obtain required UI/computer-use approval before that pass.
- Distinguish source inspection, automated assertions, actual app verification, and official reference
  observations. Report unverified items and missing credentials plainly. No invented coverage.
- Record rough request counts, transferred bytes and storage/RAM observations for the tested workload.
  These are initial measurements, not proof of free operation at 5,000 users.
- Keep a parity checklist. Do not claim "exactly Modrinth" while unresolved contract differences remain.

## Subagent coordination

The primary implementation agent owns the plan, integration, shared contracts, dev runner, decisions,
and communication with Ilai. Use subagents for separable work and independent review, not ceremony.

Suggested assignments after the inventory/runtime gate:

| Workstream         | Ownership                                                 | Handoff                                |
| ------------------ | --------------------------------------------------------- | -------------------------------------- |
| Accounts/social    | Designated account/social backend files                   | Contracts, migrations, focused tests   |
| Sharing/storage    | Designated sharing/backend and storage Rust files         | Transfer protocol, persistence, tests  |
| Integration/review | Primary initially; independent reviewer after integration | API-client/native seams, parity review |

Set exact paths before agents write. Only one owner edits shared schemas, API-client types, lockfiles,
and dev-runner files at a time. Use worktrees or non-overlapping ownership; record integration order.
Parallelize implementation only after interface agreements. Review agents should check evidence,
security boundaries, upstream drift and scope, not merely repeat the author's summary.

Child agents may delegate bounded subtasks when their environment supports it; a fresh independent
thread is not inherently required for delegation. Do not assume cross-thread messaging exists.
Choose models according to the available environment and user instructions. Ilai previously suggested
5.6-sol/high for determined mechanical work; do not make unavailable model names a blocker.

## Completion and stop conditions

Done means the three workflows pass against real local replacement services, required inherited
account/sharing operations are accounted for, persistent state survives restart, authorization and
content integrity are checked, and another developer can reproduce the environment from documented
commands. Provide changed files, focused results, known limitations, and the next bounded milestone.

Pause for a specific decision when a runtime/provider choice is unapproved, required account-provider
credentials are missing, a contract ambiguity changes product behavior, native-library edits require
permission, or a proposed UI change goes beyond compatibility. Continue unrelated safe work.
Do not solve deferred tunneling, deploy, reset user data, buy services, or publish a PR to avoid a blocker.
