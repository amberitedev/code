# Modrinth proxy lab

This package runs local substitutes for Modrinth Hosting and Shared Instances. It is an exploration tool for the real upstream App client. It is not a production backend, a compatibility promise, or evidence of how Modrinth's private services work internally.

The lab starts on `127.0.0.1:8000`. It never falls through to a private production service. Unknown routes return `501` with the method and path.

## Quick start

Install the upstream workspace dependencies once, then use the single development command:

```powershell
cd C:\Users\ilai\worktrees\modrinth-proxy-lab
corepack pnpm install --frozen-lockfile
node packages/proxy-lab/src/cli.ts dev owner --public-catalog
```

Use `recipient` instead of `owner` to open the member fixture with its seeded shared instance. Use
`--dry-run` to print the resolved paths and commands without changing data or starting processes.

`--public-catalog` is explicit because it allows anonymous `GET` and `HEAD` requests under `/v2` and `/v3` to reach `https://api.modrinth.com`. The proxy sends only `Accept` and its own user-agent; fixture authorization and cookies never reach Modrinth. Omit the flag for a fully local, fail-closed run.

The first native build can take a while and needs the normal Modrinth App prerequisites: Node and pnpm, Rust with the Windows MSVC toolchain, the WebView2 runtime, and the repository's supported Java toolchain. Later starts reuse Cargo and Vite output.

Stop with Ctrl+C. The runner records the exact children it starts and cleans up only that process tree. An exclusive run lock prevents a second invocation from touching a live fixture. It preserves all persona and service data across normal stops.

## Commands

- `node packages/proxy-lab/src/cli.ts dev [owner|recipient]` prepares the selected native fixture, starts the local backend, Vite, and the Tauri App, and owns their cleanup.
- `pnpm start -- --public-catalog` starts only the local services and optional catalog proxy.
- `pnpm reset` replaces service state with the deterministic seed and removes uploaded shared files.
- `pnpm status` prints the current Hosting and Shared Instances state summary.
- `pnpm env:prepare` creates or verifies the ignored App build environment.
- `pnpm env:clean` removes the exact generated App build environment.
- `pnpm test` runs route, persistence, WebSocket, tracing, and shared-file tests.
- `pnpm typecheck` checks the lab against the upstream API client contracts.

Every persistent runtime byte lives under the repository-root `modrinthclonedata/` directory:

- `service/` contains mock backend state, local shared uploads, and redacted HTTP traces;
- `personas/owner/` and `personas/recipient/` contain independent App databases, instances, logs, backups, and WebView2 profiles;
- `last-run.json` records the most recent process IDs and commands.

The runner adds `/modrinthclonedata/` to this checkout's local Git exclude. It refuses linked data paths, never reads Amberite `.data`, and never points `THESEUS_CONFIG_DIR`, `THESEUS_DB_BACKUP_DIR`, or `WEBVIEW2_USER_DATA_FOLDER` at a normal Modrinth profile. The fixed tokens and users are local fixtures, not real credentials.

## Implemented contract

### Hosting

The seed has one Fabric 1.21.1 server named `Proxy Lab Lighthouse` and one world. The lab implements the client routes used by these existing screens:

- server list and server detail in Archon v0 and v1;
- start, stop, restart, and kill as state transitions only;
- authenticated WebSocket console with authoritative installation state plus log, periodic stats, uptime, and power events;
- server sync SSE with initial server and network events;
- content listing, add, enable, disable, delete, bare installation state, repair, unlink, and game-version preview/apply;
- legacy backups and the backup queue for list, create, rename, restore, retry, cancel/ack, and delete;
- access list, invite, role update, reinvite, and remove;
- server properties, startup options, SFTP roll, subdomain, notices, and port allocations;
- action-log entries for simulated mutations.

No power action launches Java or Minecraft. The file browser is not implemented because it uses the separate Kyros node API. Missing server-icon reads return `404` instead of an unsupported-route error so the existing UI uses its normal fallback.

### Shared Instances

The seed has one owner, one recipient, and one ready shared version. The lab implements:

- instance create, read, rename, delete, icon state, and per-user listing;
- user list, add, and remove;
- invite-link create, inspect, accept, decline, list, and revoke;
- version publication with local upload URLs and SHA-512 checking;
- latest and numbered version reads with local download URLs;
- recipient access to published versions for the App's preview and install code;
- local fixture identity and blacklist eligibility responses.

The seeded `proxy-lab-empty.jar` is a valid empty ZIP archive. It exists to exercise the file transfer path without shipping executable mod code. It is not playable content. Published files from an App session remain inside `modrinthclonedata/service/shared-files`.

### Fixture identity and public content

The fake sign-in page returns one of two fixed local tokens to the App's existing loopback login callback. Private routes reject any other token. Traces replace authorization, cookies, and API keys with `<redacted>` and never record request bodies.

The local identity shim covers current-user reads, user lookup/search, preferences, empty notifications/friends, empty billing lists, and session refresh. Native state is pre-seeded with the selected fixture account, so exploration does not require automating a sign-in dialog. Public catalog forwarding is read-only and opt-in. All other remote writes stay local or fail.

## Assumptions and limits

The route shapes come from current `packages/api-client` modules and `packages/app-lib` call sites at upstream commit `eacc38fb51ad7ef5c66714afebd66e24b4ec9301`. Behavior that those call sites do not specify is intentionally simple:

- adding a known fixture user grants access immediately instead of modeling delivery and acceptance delays;
- backup and installation operations complete immediately;
- SSE sends a small initial snapshot and heartbeats, not the private service's full event history;
- console commands produce fixture log lines and do not execute anything;
- permission roles are enough for the current screens, not a model of private authorization policy;
- fixture uploads use local disk, with no quotas, deduplication, malware scanning, or object storage;
- public catalog reads need network access and may fail independently of the local services.

Unsupported routes return an actionable `501`. Billing changes, purchases, server provisioning, Kyros filesystem operations, SFTP, world downloads, moderation, tunnels, analytics writes, and real authenticated traffic capture are out of scope. Real traffic capture should be a separate future tool with explicit credential handling.

## Maintenance notes

Keep route changes tied to a current client call site. Prefer adding a focused test for the exact method, path, response, and state transition. Do not add a catch-all private proxy.

The App integration uses only ignored environment state and two tracked config overlays:

- `tauri.proxy-lab.conf.json` gives the dev build a separate identifier and starts Vite with the lab config.
- `vite.proxy-lab.config.ts` derives from the upstream Vite config and adds only the local HTTP and WebSocket origins to `connect-src`.

This keeps normal App source and `packages/app-lib` unchanged, which is the important property when upstream moves.
