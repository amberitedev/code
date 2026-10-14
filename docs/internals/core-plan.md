# Core design

The target design for the standalone Core (`apps/core`, Copal): what it looks like when the rewrite
on the `core-rewrite` branch is done. It replaces the earlier phased plan. Everything here is built
in one pass, in the order at the end, and merged to `main` when it is finished.

Out of scope: the Core built into the App (later), the App's UI and product logic (designed
separately; Core exposes the capabilities and the App decides how to present them), and a tunnel
for users behind CGNAT (deferred).

Based on a full read-only diagnostic on 2026-10-13 (102 findings, the 19 serious ones re-checked
against the source) and the design questions answered on 2026-10-14.

## Decisions

- Keep Rust, Axum (upgraded to 0.8), Tokio, and SQLite. Core joins the root Cargo workspace.
- Standalone Core runs as a system service on every OS, including Windows. Install needs admin
  once; nothing runs as admin after that.
- Servers run as a separate, unprivileged OS account, so a malicious mod cannot reach the owner's
  files or Core. No Docker.
- Servers run detached from Core and keep running when Core stops, crashes, or updates.
- Forge and NeoForge keep the vendor installer. Vanilla, Fabric, and Quilt use the shared Daedalus
  store. Paper stays: it fits dedicated SMP-style servers.
- Java is managed by Core only, with a per-server custom path override.
- No shared storage for mods across servers, no sleep-when-empty, no RCON.
- The owner accepts the Minecraft EULA once; Core does not start servers before that.
- Identity comes from the backend; sessions belong to Core. No ban handling: the owner's access list
  is the only thing that decides who gets in.
- Backups live on the Core machine. Off-machine backups can come later.
- The CLI is powerful but does not mirror the App: machine tasks and control, not content
  management.

## Layout

One crate, organized by domain instead of by layer:

```
src/
  main.rs        CLI entry; `copal run` starts the daemon
  daemon/        startup, config, service integration, shutdown
  servers/       server records, lifecycle, shim, console, restarts
  installs/      Daedalus store, Forge/NeoForge installer, Paper, Java
  content/       pack sync, files on disk, origins
  backups/
  access/        tickets, sessions, roles, local token, activity log
  network/       UPnP, TLS, reachability
  update/        self-updater
  api/           Axum routes, one file per resource, typed request/response structs
  db/            pool, migrations, queries
```

- No ports layer or trait-per-store. Code calls the database and the OS directly. Tests use a real
  in-memory SQLite database.
- Every API response is a typed struct, so a mismatch with `packages/api-client` is a compile error
  on the Rust side.
- One module tree, declared in `main.rs`.
- `sqlx` 0.8.6 like the rest of the repo. Migrations restart from one clean `001`. Nothing is
  deployed, so existing dev data is wiped. Shipped migrations are never edited again.
- Unused crates and dead install paths are deleted.

## Running as a service

- `copal install` runs once as admin and:
  - creates two OS accounts: `copal`, which runs Core, and `copal-servers`, which runs every server;
  - grants the one narrow permission Core needs to start servers as `copal-servers` (see below);
  - registers Core as a service running as `copal`: systemd on Linux, a Windows Service, a launchd
    daemon on macOS. The OS starts Core at boot and restarts it if it crashes.
- `copal uninstall` removes the service, the accounts, and, on request, the data.
- `copal run` runs Core in the foreground, for development and manual use.
- A lock file keeps a second copy from starting.
- Every setting has a default, so Core starts with no configuration. A config file is optional.
- Logs go to a rotating file in the data folder as well as the console.

## Server processes

Today a server is a child of Core, controlled through stdin and stdout pipes. It dies with Core, a
crash is recorded as a normal stop, and a frozen server looks healthy.

**The shim.** Core does not start Java directly. It starts a small supervisor, `copal shim`, the
same binary in a different mode, fully detached from Core and running as `copal-servers`. The shim
starts Java as its own child and owns its console: it writes stdout and stderr to `console.log` in
the server folder, passes commands from Core to Java's stdin over a local socket only Core can open,
and when Java exits it writes the exit code and time to a status file. A non-parent process cannot
read another process's exit code on Linux, and output from before Minecraft opens its own log is
otherwise lost. The shim solves both, the same way Podman's conmon and containerd's shim do.

**Isolation.** Every server runs as `copal-servers`, which can write only to server folders and
read the shared `meta` and `java` stores. It cannot read the owner's files or Core's database and
sessions. All servers share the account, so a malicious mod on one server could touch another
server's folder on the same Core; backups cover that. How Core starts the shim as that account
without admin:

- Windows: install creates the account with a random password that only `copal` can read, and Core
  uses `CreateProcessWithLogonW`, which needs no special privilege. A job object per server caps
  memory and is not set to kill on close.
- Linux and macOS: install adds a sudoers rule allowing `copal` to run only `copal shim` as
  `copal-servers`.

**Surviving Core.**

- On Linux, Core's unit uses `KillMode=process`, so stopping or updating Core never kills shims.
- On Windows, shims must not sit in a job that dies with the service.
- On startup, Core reads each server's pid file and status file and adopts every server that is
  still running instead of starting a second copy.
- Both are verified on real machines at the build checkpoint, not assumed.

**Console and commands.** Core follows `console.log` for output and sends commands through the
shim. RCON is never turned on: no extra ports, no passwords, no edits to `server.properties`.

**How a run ended.** Every run is recorded as stopped, crashed (with exit code and the last console
lines), or killed.

**Stopping.** `stop` through the shim, then wait for the process to exit, with no fixed timeout. The owner
can force-kill.

**Crash restart.** On by default, a per-server setting. After 3 crashes in 10 minutes Core stops
trying and leaves the crash report.

**Freeze detection.** Core sends a cheap command through the shim on an interval and watches for
its output. If the server stops answering for a while, it is marked frozen and the API offers a
restart. Players online come from join and leave lines in the console, checked with `list`.

**Restarts.** Two per-server settings, shown by the App under performance:

- Scheduled restarts: a daily or weekly time, with an in-game countdown.
- Smart restarts (experimental): restart only when nobody is online, after the server has run past
  an uptime threshold, preferring the hour it is usually empty, learned from join and leave history.
  Later signals feed the same rule: heap still in use after a full GC, rising over days, read from a
  per-server GC log; and tick time where the loader reports it (Paper, Forge, NeoForge, vanilla
  `/tick query` from 1.20.3).

**Memory and flags.** A small `-Xms`, the chosen `-Xmx`, and G1 settings that hand unused memory back
to the system, so an idle server shrinks. Tuned flags are the default; they are visible and
resettable per server. Memory can be changed after creation. Core reports the machine's RAM and
warns when servers together are set above it.

## Installs

- **Vanilla, Fabric, Quilt:** the shared Daedalus store in `{data}/meta/` (libraries, server jars,
  merged version JSON), installed once and reused by every server on the same version.
- **Forge, NeoForge:** the vendor installer. Fix old Forge (before 1.17) jar detection, and use
  `run.bat` on Windows.
- **Paper:** keep it, and support updating to a newer Paper build instead of pinning the one from
  install time.
- Repairing a shared install never removes files while a server is using it.
- Every download has a timeout and retries, and installs report real progress over the API.
- Server and loader updates are a supported operation, with a backup first.

**Java.** Core downloads the Java major version each Minecraft version needs (Azul Zulu, checked by
sha256) into `{data}/java/<major>`, shared by every server. On startup it checks for a newer patch.
System Java detection is removed. A server can set a custom Java path as an advanced override.

## Content

One model: content is the files on disk, plus optional metadata, with the origin of each file
recorded (from the pack, or added on the server). The separate `mods` table and its second scan go
away.

- Core tracks which files the owner edited after the pack put them there and exposes that list. A
  pack update leaves edited files alone until the App says to keep or replace them.
- Pack overrides never replace server-owned files: `server.properties`, `ops.json`, the whitelist,
  ban lists, and the world.
- Adding a mod checks what the pack already provides before installing dependencies.
- A failed pack update can always be cleared.
- Core reports available updates for the pack and for mods.

## Backups

One queued backup engine. The second backup API is removed.

- A backup is the world plus a short list of settings files, not the server folder.
- Backups run while the server is up: `save-off`, `save-all flush`, copy, `save-on` through the
  shim.
- Each server has a schedule and retention. Default: daily, plus before every pack or loader update,
  keeping the last 7. The API is general; it gets matched to the App's UI once that exists.
- Core checks free disk space first and cleans up after a failed backup.
- Backups and the current world can be downloaded through an authenticated route.

## Access

**Roles:** owner, editor, viewer. Editors can upload files and mods, which means running code on the
host machine; the App says so when inviting.

**Identity.** The backend vouches for who a user is; Core does its own checking.

1. The App asks the backend for a ticket for one Core. The ticket is a JWT signed with Ed25519,
   carrying the user id, name, avatar, and the Core id as its audience, and it expires after 5
   minutes. A ticket for one Core is useless on another Core and on the backend.
2. Core verifies the ticket with the backend's public keys, cached by key id and fetched again when
   an unknown id shows up. No backend call per request.
3. Core creates a session for that device: a random 256-bit token, stored only as a hash, tied to
   the user and a device name.

**Sessions.**

- A session expires after 30 days without use.
- The token rotates on every renewal; the old one stops working. If a rotated-out token is ever
  used, the session is treated as stolen and killed.
- Renewal does not need the backend. The backend is only needed the first time a device connects to
  a Core, and for dangerous actions.
- Removing someone from the access list ends all their sessions immediately. The owner can list
  sessions per user and device and revoke one. A user can sign out of a Core.
- Sessions are only accepted over Core's TLS, with the certificate pinned by the App.
- Token comparisons are constant-time, and the sign-in route is rate-limited.

**Dangerous actions** need a ticket from the last few minutes, or the local CLI: changing access,
transferring ownership, deleting a server, and updating Core remotely. A stolen session can run
servers but cannot add people or delete anything.

**Local access.** The CLI talks to Core over a Unix socket (Linux, macOS) or a named pipe (Windows)
that only the owner's OS account and `copal` can open. The OS checks who connects, so there is no
token to steal, and browsers cannot reach it. The same Axum routes serve both the socket and HTTPS;
a request over the socket is the owner. This is how Tailscale's CLI talks to its daemon. The CLI
works fully with the backend down. The localhost HTTPS listener checks `Host` and rejects browser
`Origin` headers, against DNS rebinding.

**Core's own key** (`amc_...`, issued at pairing, used to report Core's address) stays. A single
401 from the backend no longer wipes collaborators.

**Activity log.** Access changes, sessions created and revoked, sensitive reads, and server actions,
with who did them.

## Network

- The API is reachable from the internet. Core forwards its API port and each server's game port
  with UPnP, as today, and opens the local firewall where it can. The service account must be able
  to do this on Linux (ufw, firewalld).
- IPv6 alongside IPv4.
- When a server cannot be reached from outside, Core reports the problem and the single next step
  over the API, not only in its terminal.
- Core checks its public IP every minute and reports a change to the backend right away, with each
  server's port. Joining through the App always uses the current address.

**Stable addresses (backend, once the project domain exists).** The backend writes one SRV record
per server, `_minecraft._tcp.<random>.<domain>`, pointing at `<ip-with-dashes>.sslip.io` and the
server's port, and updates it when Core reports a new IP. Minecraft Java follows SRV records, so a
saved address keeps working and needs no port. Browsers ignore SRV records and the domain never has
an address record for a user's machine, so nobody can host a page on it and its cookies and
reputation stay safe. Names are random so they cannot be guessed to find someone's home IP. Nothing
in Core changes when this lands.

## Updates

A self-updater that feels native, because it is one of the most important parts.

- Releases are built for Windows x64, macOS arm64, Linux x64, and Linux arm64. Each release has one
  manifest listing every file's SHA-256, signed with Ed25519 (minisign format). The updater verifies
  the signature, not only HTTPS.
- Each version is installed in its own folder (`versions/<version>/`), owned by `copal`. A small
  launcher points at the active one. Updating is download, verify, repoint, restart the service. No
  running file is ever overwritten, so Windows' locked-exe problem does not arise. This is how T3
  Code and Claude Code update.
- Channels: stable, beta, alpha. Each channel resolves to Copal releases only, never a Modrinth App
  release.
- Auto-update is on by default; update on request is a setting. Updates can be triggered remotely
  from the App (a dangerous action) or from the CLI.
- Servers keep running through an update.
- The previous version is kept. If the new version fails to start, the launcher points back at the
  old one. No other automatic rollback.

## CLI

Powerful, but not a second App. It covers the machine and control of servers, and works with the
backend down. Content management (mods, packs) stays in the App.

- Core: `install`, `uninstall`, `run`, `status`, `logs`, `update` (check, apply, channel, rollback),
  `eula`
- Servers: `list`, `start`, `stop`, `kill`, `restart`, `console` (attach, send), `memory`, `java`,
  `restarts`
- Backups: `list`, `create`, `restore`, `schedule`
- Access: `list`, `role`, `remove`, `sessions`, `revoke`

It talks to Core over the local socket or pipe described under Access, so it works the same on
Windows.

## Bugs the design must close

Each of these is fixed by the design above; listed so none gets lost.

- The console reader freezes on the first non-UTF-8 line.
- Old Forge looks for `forge-server.jar`, which the installer never writes.
- Forge launch detection reads `run.sh` first on Windows.
- Repair wipes a shared install while another server uses it.
- Graceful stop kills after a fixed 30 seconds.
- Adding a mod ignores mods the pack already provides.
- Pack updates overwrite owner edits; overrides can replace `server.properties`, `ops.json`, the
  world.
- A failed pack update sticks in "failed".
- Downloads have no timeout.
- The backup download link cannot authenticate.
- Datapack buttons always return an error.
- The stats stream stops on any disk error.
- The stable channel can pick a Modrinth App release; beta and alpha never resolve.
- The Linux service cannot open the firewall.
- `uninstall` removes nothing.

## Build order

Built in this order, in small commits that each compile. Real servers are started at the marked
checkpoints, not only at the end.

1. **Foundation:** workspace, `sqlx` 0.8.6, Axum 0.8, new layout, clean migrations, config
   defaults, typed API structs, dead code removed.
2. **Service and processes:** `copal install` (accounts, permission, service) and `run`, the shim,
   isolation, adoption, console, run outcomes, stopping, the local socket. *Checkpoint: start,
   restart Core, adopt, stop, on Windows and Linux; a server cannot read the owner's files.*
3. **Access:** tickets and the key endpoint in `apps/backend`, sessions in Core, the ticket flow in
   `packages/api-client`, dangerous actions, activity log. Merge `main` in first.
4. **Installs and content:** Java, Forge and Paper fixes, downloads, the single content model, edited
   files. *Checkpoint: one server per loader, plus a pack update with an edited config.*
5. **Backups:** engine, hot backups, schedule, retention, download.
6. **Updater and releases:** four targets, signing, channels, rollback. *Checkpoint: update Core
   with servers running.*
7. **CLI:** the full command set.
8. **Reliability and insight:** crash restart, freeze detection, players, memory and flags,
   scheduled and smart restarts, EULA, reachability reporting, IPv6.

## Testing

Focused tests as each step lands, against an in-memory SQLite database: migrations, access and
sessions (rotation, reuse detection, revocation, dangerous actions), content origins and edited
files, backup retention, and run outcome recording.
