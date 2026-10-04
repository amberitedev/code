# Local account and sharing verification

Work in progress, September 24, 2026, on `restore-account-sharing`.

## Baseline

The launcher restoration at `387a96f58` was compared with upstream `10b1d7002`.
The nine differing App files preserve development bootstrap, configuration, browser bridge, and
installation isolation. Restoration uses that compatible client baseline rather than mixing newer
App code with older shared packages. Current toolchain and package configuration are retained.

The inherited native account database, friends socket, and sharing/install workflows remain in use.
Private endpoints route to the local service; public content and explicit public creator profiles
route to Modrinth. Custom native transport and recovery code lives under `self_hosted/`; independent
storage code lives under `storage/`, with a `sharing-storage` executable.

## Verified so far

- Restored App frontend production bundle builds, including public creator profile routing.
- Native launcher library and desktop executable compile with the existing native environment.
  Both isolated desktop scenarios start. The command returning Minecraft accounts retains the
  current library's account-summary response rather than exposing credential records.
- API-client routing tests pass: local account/sharing routes, public credential isolation, explicit
  sign-in token handling, anonymous login, and local authentication globals.
- Nineteen focused dev-runner tests pass, including copied-database path isolation. Before spawning
  a scenario, the runner checks that it is stopped and rebases its saved launcher paths into that
  scenario's worktree directory. The fixture verifies that the source database remains unchanged.
- Real local Worker and D1 account tests pass for password signup/login, denied invalid credentials,
  profile permissions, friend request/acceptance, blocking, session rotation/revocation, TOTP and
  backup codes, notification privacy, and friends socket presence/disconnection.
- The same account tests pass after stopping and restarting the backend with its persistent data.
- Fresh local D1 migration tests pass, including sharing version idempotency and permission checks.
- All three native journal tests pass. They cover changed snapshot/destination detection, the crash
  between successful upload and local state reconciliation, and preserving metadata update order,
  with bounded, account-scoped recovery queries.
- Rust storage tests cover interrupted/wrong-hash upload rejection, idempotent retry, committed-file
  corruption detection, and range parsing. Native download integrity tests reject altered bytes even
  when their length is unchanged.
- The real HTTP sharing proof passes against the local Worker/D1 and both Rust storage processes:
  friendship and invite acceptance, version 1 upload/download, version 2 update/download, wrong-hash
  rejection, interrupted PUT and GET retries, and byte ranges. Version 1 transferred 524,323 bytes;
  version 2 transferred 524,335 bytes, with SHA256 checked against the source.
- With the owner's session revoked, recipient download and direct second-node replication succeed.
  Equal-length corruption on the first node falls back to the intact second copy. After both copies
  are removed, download reports unavailable; uploading the retained owner snapshot restores access.
  Unrelated accounts and removed recipients cannot resolve the metadata download capability.
- The timed HTTP run completed at `2026-09-24T10:42:30Z` using instance
  `da734006-8122-4c8e-a1da-fd1679518c5f`, versions 1 and 2. Its report and both immutable snapshots
  are under `.data/sharing-proof/mufekh0rcedab9/`. Verified upload took 109 ms, owner-offline download
  62 ms, and version 2 download 78 ms. The second copy was observed 6,533 ms after the first upload
  completed; that includes heartbeat scheduling and intervening checks, not only transfer time.
  The measured scenario took 7,684 ms after initial health checks and owner login. These are one
  localhost run's measurements, not an App latency or network benchmark.
- The HTTP fixture is synthetic bytes labeled as a resource pack, so this proves storage transport
  and contracts independently of native installation.
- Two real native Apps signed in through the local website's password form and native callback as
  distinct D1 accounts. Their active SQLite account IDs are `TopoNC3E` and `khCGzq9P`. The owner sent
  a friend request through the App; the recipient accepted it and both Apps showed the other online.
- The owner shared `Local sharing proof`, a Vanilla 1.21.1 instance with a valid 387-byte resource
  pack. The recipient accepted the notification, reviewed the external-file warning, and installed
  through the existing App dialog. Its native install job succeeded in 93 seconds, including Java
  and 876.4 MiB of Minecraft downloads. The recipient pack's SHA256 matches the original fixture:
  `ee27445ff30a70d6da1b30cf7d149f93cabd446164b77c588aa594ed1398e587`.
  Both installation paths are inside their separate worktree scenario directories. Evidence lives
  under `.data/app-proof/20260924/`; shared instance ID is `fc76d722-1906-4c02-9322-ebd11df0a8a5`.

## Outstanding integrated proof

The initial two-App friend/share/install workflow passes. Required steps remain owner update,
recipient update, account switching/restart, owner-offline native download, and native interrupted
transfer retry. The changed-file check exposed an inherited filename-only preview comparison;
a same-name, same-size pack with changed bytes incorrectly showed no changes. Its fix is in progress.

The runtime storage proof is in `apps/backend/test/sharing-storage.mjs`; real account tests are in
`apps/backend/tests/accounts.mjs`. These run against the worktree's services and do not deploy.
Test artifacts and credentials stay under the gitignored `.data/` directory.

## Deliberately deferred or incomplete

Encryption is explicitly deferred. Storage operators can read the current plain files. The future
requirement is encryption that prevents those operators from reading contributed content.

External OAuth, outbound email delivery, passkeys, PATs, third-party OAuth authorization, and a
production captcha provider are not implemented. Local email flows use an outbox. Non-local password
authentication fails with a configuration error until its captcha provider is configured.

Private-account/shared-instance reports and report attachments return an unsupported error locally;
they are not sent to Modrinth. Upstream analytics and support telemetry are disabled when the
self-hosted account service is configured. Invalid launcher callback parameters fail locally rather
than forwarding a self-hosted session to the upstream launcher callback site.

The backend checks membership again when resolving a download. An already-issued storage-node
download capability has a bounded expiry and is not instantly revoked. Storage garbage collection
after metadata version pruning is not yet implemented. No Minecraft management, tunnels, cloud
deployment, or cloud billing setup is part of this proof.
