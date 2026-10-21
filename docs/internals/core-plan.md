# Core design

The target design for the standalone Core (`apps/core`, Copal): what it looks like when the rewrite
on the `core-rewrite` branch is done. It replaces the earlier phased plan. Everything here is built
in one pass, in the order at the end, and merged to `main` when it is finished.

Out of scope: the Core built into the App (later), the App's UI and product logic (designed
separately; Core exposes the capabilities and the App decides how to present them), and a tunnel
for users behind CGNAT (deferred).

Based on a full read-only diagnostic on 2026-10-13 (102 findings, the 19 serious ones re-checked
against the source), the design questions answered on 2026-10-14, and the security and access
rethink of the same day (reviewed by GPT-6.1 Sol; the points taken are folded in below).

## Decisions

- Keep Rust, Axum (upgraded to 0.8), Tokio, and SQLite. Core joins the root Cargo workspace.
- Standalone Core runs as a system service on every OS, including Windows. Install needs admin
  once; nothing runs as admin after that.
- Two OS accounts: Core and its shim run as `copal`; Java, installers, and any custom Java path run
  as `copal-servers`. A malicious mod owns the servers on that machine. It cannot reach Core's
  database, keys, backups, binaries, or the owner's files. No Docker.
- Servers run detached from Core and keep running when Core stops, crashes, or updates.
- Forge and NeoForge keep the vendor installer. Vanilla, Fabric, and Quilt use the shared Daedalus
  store. Paper stays: it fits dedicated SMP-style servers.
- Java is managed by Core only, with a per-server custom path override.
- No shared storage for mods across servers, no sleep-when-empty, no RCON.
- The owner accepts the Minecraft EULA once; Core does not start servers before that.
- Identity comes from the backend as a short signed ticket. Sessions belong to Core. Core is the
  only authority on who can do what; the backend holds a directory copy that Core rewrites.
- Roles are per server, not per Core. A friend helps with one server and never sees the others.
- The backend is a phone book and a notifier, nothing more. Core talks to it on start and on
  change, never on a timer. The whole project must fit Cloudflare's free tier.
- Backups live on the Core machine. Off-machine backups can come later.
- The CLI is powerful but does not mirror the App: machine tasks and control, not content
  management.
- There are no users. Nothing migrates from the current Core. Nothing is kept working for its sake.

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
  access/        tickets, sessions, roles, local socket, activity log, backend sync
  network/       UPnP, TLS, reachability
  update/        self-updater
  api/           Axum routes, one file per resource, typed request/response structs
  db/            pool, migrations, queries
```

- No ports layer or trait-per-store. Code calls the database and the OS directly. Tests use a real
  in-memory SQLite database.
- Every API response is a typed struct. For the Amberite-only routes (access, sessions, activity,
  backend sync) the TypeScript types in `packages/api-client` are generated from the Rust structs
  with `ts-rs`, so a mismatch is a build error on both sides. The hosting routes must match
  Modrinth's upstream shapes, which already exist in `api-client`; those are checked at the
  integrated checkpoints, not by the compiler.
- One module tree, declared in `main.rs`.
- `sqlx` 0.8.6 like the rest of the repo. Migrations restart from one clean `001`. Shipped
  migrations are never edited again.
- Unused crates and dead install paths are deleted.

**Data folder.** The isolation story is this table. It is written in step 1 so step 2 cannot get it
wrong.

| Folder           | Owner   | `copal-servers` | Holds                                               |
| ---------------- | ------- | --------------- | --------------------------------------------------- |
| `db/`            | `copal` | no access       | database, sessions, Core's backend key, TLS key     |
| `backups/`       | `copal` | no access       | every backup                                        |
| `versions/`      | `copal` | no access       | installed Core versions                             |
| `sockets/`       | `copal` | no access       | shim command sockets                                |
| `runs/<server>/` | `copal` | no access       | pid, run id, status, `console.log`                  |
| `servers/<id>/`  | `copal` | read and write  | world, mods, config: the only place a mod can write |
| `meta/`, `java/` | `copal` | read only       | server jars, libraries, Java runtimes               |

## Running as a service

- `copal install` runs once as admin and:
  - creates two OS accounts, `copal` and `copal-servers`, both denied interactive logon so they
    never appear on a login screen;
  - records the id of the account that ran install as the owner for the local socket (explicitly,
    so `sudo` or SSH cannot confuse it);
  - grants the narrow rights Core needs to start servers as `copal-servers` (see below);
  - opens the firewall once for Core's port and the server port range (Windows Firewall, ufw,
    firewalld). Core never runs a firewall tool later, so it needs no permission to;
  - registers Core as a service running as `copal`: systemd on Linux, a Windows Service, a launchd
    daemon on macOS. The service runs the launcher (see Updates), which runs Core. The OS starts
    it at boot and restarts it if it crashes.
- `copal uninstall` removes the service, the accounts, the firewall rules, and, on request, the
  data.
- `copal run` runs Core in the foreground, for development and manual use.
- A lock file keeps a second copy from starting.
- Every setting has a default, so Core starts with no configuration. A config file is optional.
- Logs go to a rotating file in the data folder as well as the console.

## Server processes

Today a server is a child of Core, controlled through stdin and stdout pipes. It dies with Core, a
crash is recorded as a normal stop, and a frozen server looks healthy.

**The shim.** Core does not start Java directly. It starts a small supervisor, `copal shim`, the
same binary in a different mode, fully detached from Core and running as `copal`. The shim starts
Java as `copal-servers` as its own child and owns its console: it writes stdout and stderr to
`runs/<server>/console.log`, passes commands from Core to Java's stdin over a local socket only
`copal` can open, and when Java exits it writes the exit code and time to a status file. A
non-parent process cannot read another process's exit code on Linux, and output from before
Minecraft opens its own log is otherwise lost. The shim solves both, the same way Podman's conmon
and containerd's shim do.

The shim runs as `copal`, not as the mod's account. With the shim on `copal-servers`, a mod on
server A could open server B's command socket and type `/op`, forge B's pid file so Core kills the
wrong process, and write fake lines into the console log Core reads. The shim never opens a path
inside `servers/` as a control file, so a symlink planted by a mod cannot redirect its writes.

**Isolation.** Every server runs as `copal-servers`, which can write only to server folders and
read the shared `meta` and `java` stores. The Forge and NeoForge installers, `run.bat`, and a
custom Java path run as `copal-servers` too, because they execute code. All servers share the
account, so a malicious mod on one server can touch another server's folder on the same Core;
backups cover that. How the shim starts Java as that account without admin:

- Windows: install creates the account with a random password, stored with DPAPI under the `copal`
  profile so only that account can decrypt it. The shim uses `LogonUser` (batch logon) then
  `CreateProcessAsUser`, the documented pattern for services. Install grants `copal` the two
  privileges that needs ("replace a process level token", "adjust memory quotas") and
  `copal-servers` the "log on as a batch job" right, through the LSA policy API.
  `CreateProcessWithLogonW` is the fallback; both are tried on a real Windows machine at the
  step-2 checkpoint before anything is built on top. Java gets a temp folder inside the server
  folder, because an account without a loaded profile has no usable `%TEMP%`. A job object per
  server caps memory and is not set to kill on close.
- Linux and macOS: one file in `sudoers.d`, one line, an absolute path to one fixed helper, no
  wildcards in the user part. It only lets `copal` become `copal-servers`, a step down, so even a
  mistake cannot become root.

**Surviving Core.**

- On Linux, Core's unit uses `KillMode=process`, so stopping or updating Core never kills shims.
- On Windows, shims must not sit in a job that dies with the service.
- On startup, Core reads each server's run files and adopts every server that is still running
  instead of starting a second copy.
- Process identity is more than a pid. The shim records a run id, the pid, the process start time,
  and the machine's boot id. Core adopts or kills only when all of them match. One lock file per
  server, so a Core that crashes mid-start cannot launch the same server twice.
- The shim protocol is versioned. Shims outlive Core, so a new Core meets old shims after an update
  and an old Core meets new ones after a rollback. Every message carries a version; a shim Core
  cannot understand is left running and shown as "restart to upgrade".
- Both are verified on real machines at the build checkpoint, not assumed.

**Console and commands.** Core follows `console.log` for output and sends commands through the
shim. Console output is untrusted text: join and leave lines are parsed with strict patterns, lines
are capped at 64 KB, and logs are capped per run. RCON is never turned on.

**How a run ended.** Every run is recorded as stopped, crashed (with exit code and the last console
lines), or killed.

**Stopping.** `stop` through the shim, then wait for the process to exit, with no fixed timeout. The
owner can force-kill.

**Crash restart.** On by default, a per-server setting. After 3 crashes in 10 minutes Core stops
trying and leaves the crash report.

**Freeze detection.** Core sends a cheap command through the shim on an interval and watches for
its output. The reply must echo a random word, so a mod cannot fake "I am alive" cheaply. If the
server stops answering for a while, it is marked frozen and the API offers a restart. Players
online come from join and leave lines in the console, checked with `list`.

**Restarts.** Two per-server settings, shown by the App under performance:

- Scheduled restarts: a daily or weekly time, with an in-game countdown.
- Smart restarts (experimental): restart only when nobody is online, after the server has run past
  an uptime threshold. That is the whole rule. GC heap trends and tick time are cut; they are a
  product feature wearing an engine costume.

**Memory and flags.** A small `-Xms`, the chosen `-Xmx`, and G1 settings that hand unused memory back
to the system, so an idle server shrinks. Tuned flags are the default; they are visible and
resettable per server. Memory can be changed after creation. Core reports the machine's RAM and
warns when servers together are set above it.

## Installs

- **Vanilla, Fabric, Quilt:** the shared Daedalus store in `{data}/meta/` (libraries, server jars,
  merged version JSON), installed once and reused by every server on the same version.
- **Forge, NeoForge:** the vendor installer, run as `copal-servers`. Fix old Forge (before 1.17)
  jar detection, and use `run.bat` on Windows.
- **Paper:** keep it, and support updating to a newer Paper build instead of pinning the one from
  install time.
- Repairing a shared install never removes files while a server is using it.
- Every download has a timeout and retries, and installs report real progress over the API.
- Server and loader updates are a supported operation, with a backup first.

**Downloads only from known hosts.** Core downloads pack content only from the hosts Modrinth
allows in a pack (`cdn.modrinth.com`, `github.com`, `raw.githubusercontent.com`, `gitlab.com`) and
re-checks the host on every redirect. It resolves the name first, rejects private addresses, and
connects to the address it checked, so DNS tricks do not help.

**Java.** Core downloads the Java major version each Minecraft version needs (Azul Zulu, checked by
sha256) into `{data}/java/<major>`, shared by every server. On startup it checks for a newer patch.
System Java detection is removed. A server can set a custom Java path as an advanced override; it
runs as `copal-servers` like everything else that executes.

## Content

One model: content is the files on disk, plus optional metadata, with the origin of each file
recorded (from the pack, or added on the server). The separate `mods` table and its second scan go
away.

- Core tracks which files the owner edited after the pack put them there and exposes that list. A
  pack update leaves edited files alone until the App says to keep or replace them.
- Pack overrides and uploads never replace server-owned files: `server.properties`, `ops.json`, the
  whitelist, ban lists, `eula.txt`, and the world. A deny-list of server-owned files beats an
  allow-list of folders because packs legitimately touch many folders.
- Every archive entry must land inside the server folder. Entries with `..`, absolute paths, or
  symlinks are rejected, and unpacked size is capped.
- Adding a mod checks what the pack already provides before installing dependencies.
- A failed pack update can always be cleared.
- Core reports available updates for the pack and for mods.

## Backups

One queued backup engine. The second backup API is removed.

- A backup is the world plus a short list of settings files, not the server folder. Backups live in
  `backups/`, owned by `copal`, out of any mod's reach.
- Backups run while the server is up: Core asks the shim to hold `save-off` for at most ten
  minutes, runs `save-all flush`, copies, then releases. If Core never comes back, the shim sends
  `save-on` itself. The shim outlives Core, so autosave can never be left off.
- Each server has a schedule and retention. Default: daily, keeping the last 7. Pre-update backups
  keep their own retention (last 2) so they do not evict the daily ones. The API is general; it
  gets matched to the App's UI once that exists.
- Core checks free disk space first and cleans up after a failed backup.
- Backups and the current world can be downloaded through an authenticated route.
- Mods that write outside the save cycle can still produce a slightly inconsistent hot backup.
  Documented, not solved.

## Access

**Who we defend against.** A random person on the internet (no session without a ticket; limits).
A friend who goes rogue or loses a laptop (roles; removal kills every session). A malicious mod
(separate OS account; still owns the servers, accepted). A stolen Core session (can run servers,
cannot change access, delete, restore, or update Core). Malware on the owner's own PC is the owner;
out of scope. A compromised backend can mint a ticket as anyone; every "sign in with X" system has
this property, so the backend stays small and every Core keeps working with it down.

**Roles, per server.** The owner is per Core and has everything. Friends never see the Core, only
the servers they were given.

| On a server                                                                      | Viewer | Editor | Owner |
| -------------------------------------------------------------------------------- | ------ | ------ | ----- |
| See status, players, content list, backup list                                   | yes    | yes    | yes   |
| Start, stop, restart, kill                                                       |        | yes    | yes   |
| Console: read and send                                                           |        | yes    | yes   |
| Upload files and mods, pack updates, settings, memory, Java path (runs code)     |        | yes    | yes   |
| Create and download backups, download the world                                  |        | yes    | yes   |
| Restore a backup (ticket)                                                        |        |        | yes   |
| Invite, remove, change roles (ticket)                                            |        |        | yes   |
| Delete the server (ticket)                                                       |        |        | yes   |
| On the Core: create servers, EULA, settings, update Core, sessions, activity log |        |        | yes   |

Viewers do not get the console; it carries chat and player IPs. Editors run code on the host
machine, and the App says so when inviting. Every route checks the role on every call. There is no
"accepted the invite" state; invite means granted.

**Identity: tickets.** The App never hands its account session to a Core.

1. The App asks the backend for a ticket for one Core. The backend checks the account session and
   signs a JWT with Ed25519 carrying issuer, user id, name, avatar, the Core id as audience, issue
   and expiry times (5 minutes), and a random one-time id. The backend stores nothing: one request,
   two reads, zero writes.
2. The App sends the ticket to that Core over pinned TLS, with a device name.
3. Core verifies the signature with the backend's public key it already holds. Algorithm pinned to
   EdDSA, nothing else accepted. Issuer and Core id checked. Sixty seconds of clock leeway, with an
   error that says "your clock is wrong" when that is the cause. The one-time id is stored in the
   same database write that uses it, so a ticket can never make two sessions or authorize two
   actions. Then Core checks its own access list. Not on it: no session, nothing stored.
4. Core answers with a random 256-bit session token. It stores only a hash. The App keeps the token
   in its Rust side, in the OS keychain. The WebView never sees it.

**Keys.** The backend publishes its public keys, each with an id, as a static file (Workers static
assets are free and do not count against the request limit). Core stores them in its database,
refreshes once a day, and refetches on an unknown id at most once per ten minutes. Rotation:
publish the new key, keep the old one listed for a day, then drop it.

**Sessions.**

- One session per device per Core, named. 30 days without use, one year at most. Each use extends
  it; Core writes "last used" at most once an hour.
- No rotation. A dropped reply during renewal looks exactly like theft, and the token only lives in
  the keychain and only travels over pinned TLS.
- Removing someone from a server ends their sessions and live console streams at once. The owner
  can list sessions per user and device and revoke one (a dangerous action for someone else's). A
  user can sign out of a Core.
- Only accepted over Core's TLS: a self-signed ten-year certificate, never renewed, pinned by the
  App through the backend. Cloning the data folder onto a second machine is unsupported.
- Token comparisons are constant-time. The sign-in route takes 10 per minute per address.

**Dangerous actions** need a ticket under five minutes old, used once, whose user matches the
session's user, or the local CLI: inviting, removing, or changing someone's role; deleting a
server; restoring a backup over a world; revoking someone else's session; updating Core from the
App. A stolen session can run servers but cannot change who gets in or destroy anything. With the
backend down these are CLI-only; everything else keeps working. A ticket proves the device still
holds the Amberite account, not that someone just typed a password; that is the limit.

**Local access.** The CLI talks to Core over a Unix socket (Linux, macOS) or a named pipe (Windows)
that only `copal` and the owner's OS account recorded at install may open. The OS checks who
connects, so there is no token to steal, and browsers cannot reach it. The pipe is created with
"reject remote clients". The same Axum routes serve both the socket and HTTPS; a request over the
socket is the owner. This is how Tailscale's CLI talks to its daemon. The CLI works fully with the
backend down. The HTTPS listener checks `Host` and rejects browser `Origin` headers, against DNS
rebinding. There is no plain HTTP listener. Nice to have: the CLI checks that the process on the
other end of the pipe is `copal`, against pipe-name squatting.

**One list, not two.** Core keeps `access(server_id, user_id, role)`, the only list that decides
anything. The backend keeps `core_access(core_id, user_id)`: who may be told this Core's address
and certificate, nothing about roles. The owner is known from sign-in and never in the table.

- Sync: Core keeps a small "owed to the backend" list of calls it still has to make, retried with
  backoff. On every start and reconnect it also sends its whole directory list in one `PUT` that
  replaces the backend's copy, the way the state report replaces its server list. A full replace cannot drift.
- Invite: the owner invites a user to a server with a role, with a fresh ticket. Core adds the row
  and owes the backend one call: this user may reach me, notify them. Accepting is just connecting.
  Only the owner may invite; today's backend lets anyone with access do it, and that goes.
- Remove: Core deletes the row and kills the sessions. If the user has no other server on this
  Core, Core owes the backend "drop the directory entry".
- Remove a Core from the account: the App tells the Core first, as a dangerous action, then the
  backend. If the Core was unreachable, it learns at its next start, when its sync call is refused.

**Core's own key** (`amc_...`, issued at pairing) stays for Core's own calls to the backend. A
single 401 from the backend no longer wipes anything.

**Activity log.** Access changes, sessions created and revoked, sensitive reads, and server actions,
with who did them.

**Limits on the public listener.** Bodies 10 MB except upload routes, which stream to disk with a
per-server cap. 30-second request timeout, idle timeout, caps on open connections and on live
console streams per session, 64-character device names.

## Network

- The API is reachable from the internet. Core forwards its API port and each server's game port
  with UPnP, as today. The firewall was opened at install.
- Port mappings belong to servers, not to Core. Core removes a mapping only when that server stops,
  never when Core itself exits, and re-adopts mappings for adopted servers on start.
- IPv6 alongside IPv4.
- When a server cannot be reached from outside, Core reports the problem and the single next step
  over the API, not only in its terminal.
- Core learns its public IP from the router (UPnP has a call for it), falling back to a public STUN
  server. Never from the backend. It checks locally every few minutes and reports to the backend
  only when the answer changes.

**Stable addresses (backend, once the project domain exists).** The backend writes one SRV record
per server, `_minecraft._tcp.<random>.<domain>`, pointing at `<ip-with-dashes>.sslip.io` and the
server's port, and updates it when Core reports a new IP. Minecraft Java follows SRV records, so a
saved address keeps working and needs no port. Browsers ignore SRV records and the domain never has
an address record for a user's machine, so nobody can host a page on it and its cookies and
reputation stay safe. Names are random so they cannot be guessed to find someone's home IP. Nothing
in Core changes when this lands.

## Backend

Cloudflare free tier: 100,000 Worker requests a day; D1 5 million rows read and 100,000 rows
written a day. Writes run out first, and every "update last seen" is a write. The rule: the backend
is a phone book and a notifier. No timers.

- **No heartbeat.** Core reports on start, on address change, on server list change, and on access
  change. One combined "here is my state" call on start. A Core that sits there costs zero requests
  a day. The backend writes only when a field changed.
- **Online status comes from the App.** It has the address and connects directly. If a Core does
  not answer, the App asks the backend once for a fresh address. Reads are the cheap resource.
- **Reachability test** is a button the owner presses, not a timer, and it does not write a
  counter row per attempt.
- **Cleanup.** A Core that has not reported in a year is deleted.
- **Tickets** cost one request and two reads, zero writes. One per device per Core, then one per
  dangerous action. Public keys are a static asset, free.
- **The App's Core list** is read once at start and cached, re-read only when a Core is unreachable.

Rough total for one owner with one Core and two friends: a handful of requests a day. Headroom is
tens of thousands of Cores, against a few hundred with today's five-minute heartbeat.

## Updates

A self-updater that feels native, because it is one of the most important parts.

- Releases are built for Windows x64, macOS arm64, Linux x64, and Linux arm64. Each release has one
  manifest listing every file's SHA-256, plus version, channel, and target, signed with Ed25519
  (minisign format). The updater verifies the signature, not only HTTPS.
- Each version is installed in its own folder (`versions/<version>/`), owned by `copal`. A small
  launcher points at the active one. Updating is download, verify, repoint, restart the service. No
  running file is ever overwritten, so Windows' locked-exe problem does not arise. This is how T3
  Code and Claude Code update. The launcher binary itself is never replaced by the updater; a full
  reinstall covers it.
- Downgrades are refused unless the CLI says `--allow-downgrade`, so a validly signed old manifest
  cannot push a Core back to a buggy build.
- Channels: stable, beta, alpha. Each channel resolves to Copal releases only, never a Modrinth App
  release.
- Auto-update is on by default; update on request is a setting. Updates can be triggered remotely
  from the App (a dangerous action) or from the CLI.
- Servers keep running through an update. Old shims keep working through the versioned protocol.
- "Healthy" is defined: the new version writes a marker after it answers its first request, within
  60 seconds. No marker, the launcher points back at the previous version, which is kept. The
  database file is copied aside before migrating; rolling back restores it. No other automatic
  rollback.
- The launcher runs the active version as its child and starts it again when it exits to update,
  on every OS and in development, so an update never needs the service manager: Core just exits.
  `copal run` is the launcher; the service manager restarts it only if it dies itself.

## CLI

Powerful, but not a second App. It covers the machine and control of servers, and works with the
backend down. Content management (mods, packs) stays in the App.

- Core: `install`, `uninstall`, `run`, `status`, `logs`, `update` (check, apply, channel, rollback),
  `eula`
- Servers: `list`, `create`, `delete`, `start`, `stop`, `kill`, `restart`, `console` (attach, send),
  `memory`, `java`, `restarts`
- Backups: `list`, `create`, `restore`, `schedule`
- Access: `list`, `role`, `remove`, `sessions`, `revoke`

It talks to Core over the local socket or pipe described under Access, so it works the same on
Windows.

## Cut, and why

- **Token rotation.** Adds a failure mode, removes nothing real.
- **Ownership transfer.** It has to happen on the backend and on Core at once, with both accounts
  agreeing. "Remove the Core from my account, the new owner pairs it" does the same job with zero
  new code. Servers and files stay on disk through it.
- **Smart-restart extra signals** (GC heap trends, tick time). The uptime rule stays.
- **Plain HTTP on the LAN.** The socket replaced its only user.
- **Backend as "what is my IP".** Router or STUN instead.
- **Periodic heartbeat.** Change-only reports do the job for free.
- **Pending-invite state.** Invite means granted.
- **Migration from the current Core.** No users.

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
- The App hands its whole account session to every Core, friends' included.
- Port mappings are removed when Core exits, while the servers keep running.
- Anyone with access to a Core can invite others.

## Build order

Built in this order. Real servers are started at the marked checkpoints, not only at the end.

1. **Foundation:** workspace, `sqlx` 0.8.6, Axum 0.8, new layout, the data folder table, clean
   migrations, config defaults, typed API structs with `ts-rs` for the Amberite-only routes, dead
   code removed.
2. **Service and processes:** `copal install` (accounts, rights, firewall, service) and `run`, the
   shim as `copal` with state in `runs/`, Java as `copal-servers`, run id and lock, versioned
   protocol, adoption, console, run outcomes, stopping, the local socket. _Checkpoint on a real
   Windows and Linux machine: start, restart Core, adopt, stop, force-kill; TLS up; a process
   running as `copal-servers` cannot read `db/` or open another server's socket; macOS if a Mac is
   around._
3. **Access.** Backend first, on `main`: ticket endpoint, public key file, change-only state
   report, full-replace `PUT /core/access`, owner-only invites. These are additive and the current
   Core ignores them. Then Core: sessions, per-server roles, dangerous actions, activity log,
   socket identity, the owed-calls list. Then `packages/api-client`: the ticket flow, token in the
   keychain, "remove Core" tells Core first. The account-token lookup in Core is deleted with no
   fallback.
4. **Installs and content:** Java, Forge and Paper fixes, download allowlist and redirect checks,
   archive checks, installers as `copal-servers`, the single content model, edited files.
   _Checkpoint: one server per loader, plus a pack update with an edited config._
5. **Backups:** engine, shim-held hot backups, schedule, retention, pre-update retention,
   download.
6. **Updater and releases:** four targets, signing, channels, the five guards. _Checkpoint: update
   Core with servers running._
7. **CLI:** the full command set.
8. **Reliability and insight:** crash restart, freeze detection, players, memory and flags,
   scheduled and smart restarts, EULA, reachability reporting, IPv6.

Steps 1 to 3 are sequential. After 3, steps 4 and 5 run in one lane and 6, 7, and 8 in another, in
separate worktrees.

## How it ships

- `core-rewrite` is the integration branch. Each step is one PR into it from a short branch, so
  CodeRabbit reviews the code (`.coderabbit.yaml` lists `core-rewrite` as a base branch).
  CodeRabbit skips any PR over 100 changed files, so a step that would pass that splits into two
  PRs. Docs and config changes go straight onto the branch.
- Backend and `api-client` changes in step 3 go to `main` as their own PRs and are deployed, so the
  new Core is tested against the real backend and the final merge is mostly Rust.
- Gate per PR: CI green, every CodeRabbit finding fixed or dismissed with a written reason, a
  second-opinion review by GPT-6.1 Sol run as a subagent on every PR, and a read by the agent
  driving the rewrite. Sol's findings are judged, not merged: each one is taken or rejected with a
  reason.
- Parity checkpoint: after step 8 the owner installs the new Core on their own machine and uses it
  for a few days. That is the acceptance test.
- Final: one PR switches the App to tickets and merges `core-rewrite` into `main`. Until then the
  UI work on `main` moves the Core screens around but does not rewire what they call.

## Testing

Focused tests as each step lands, against an in-memory SQLite database: migrations, tickets (bad
algorithm, wrong Core, expired, reused), sessions (expiry, revocation, dangerous actions), per-server
roles on every route, the owed-calls list and full replace, content origins and edited files,
archive and download checks, backup retention, and run outcome recording.
