# Core plan

What Core (`apps/core`) needs to become a real replacement for paid Minecraft hosting, in the order
we intend to do it. Based on a full read-only diagnostic on 2026-10-13: 102 findings across seven
subsystems, with the 19 serious ones re-checked against the source (15 confirmed, 4 partly right,
none refuted).

Order of work: architecture first, then the bugs that sit on top of it, then features.

## Decided

- Keep Rust, Axum, Tokio, and SQLite.
- Forge and NeoForge keep using the vendor installer.
- No Docker.
- No shared storage for mods across servers.
- Servers run detached from Core and are adopted again after a restart (the full process redesign).
- Core checks who a user is itself, from a short-lived token the backend signs for that one Core.
- Windows is a required target. Core already runs on Windows in development; what is missing is a
  release build, an installer, and a way to keep it running.
- Core joins the root Cargo workspace.

## Deferred

- Tunnel fallback for users behind CGNAT or a locked router. Not part of this plan.

## Where Core stands

| Subsystem | State | Why |
|---|---|---|
| Install and launch | Shaky | Installs are sound for Vanilla, Fabric, Quilt, Paper. The process layer has no supervision and Forge has real bugs. |
| Content sync | Mostly solid | The pack sync engine is the best part of Core. It overwrites owner edits and has a second, overlapping mod system beside it. |
| API | Functional | The main loop works. Several panel features are stubs or return fake data. |
| Files and backups | Half-built | Path safety is good. Backups while running cannot work, and there is no schedule or retention. |
| Auth and access | Needs redesign | Permission checks are solid. The way users prove who they are is not. |
| Networking | Sound design | Thorough when a direct connection is possible. No fallback when it is not. |
| Foundation | Needs rework | Will not start without hand-set config, Linux only, update path has bugs, almost no tests. |

## Phase 1: architecture

Each item is one pull request or a short series. They are ordered so later ones build on earlier
ones.

### 1.1 Foundation cleanup

No behavior change. Makes everything after it easier to read and review.

- Move `sqlx` from `0.9.0-alpha.1` to the stable `0.8.6` the rest of the repo uses. Core uses no
  0.9-only API; the pin came in with the first import and was never explained.
- Delete unused crates and the dead install paths (`install_quilt`, `resolve_vanilla`,
  `resolve_fabric`).
- Delete the ports layer: three store traits and the spawner traits, each with one implementation,
  plus a mock no test uses. About 100 queries already bypass them.
- Delete `presentation/handlers/`. No route reaches it. `hosting/` calls its functions by building
  fake request objects.
- Split `hosting/http.rs` by resource and replace hand-built JSON responses with typed structs, so a
  mismatch with the client contract is a compile error.
- Stop declaring the module tree in both `lib.rs` and `main.rs`, which compiles everything twice.
- Reset migrations to a clean set. Nothing is deployed, so remove the adoption block and the
  edited-in-place first migration, and never edit a shipped migration again.
- Join the root workspace, with a release profile for Core that does not abort on
  panic.

### 1.2 Process lifecycle

The largest change, and the one most features wait on.

Today a server is a child process of Core, controlled through its stdin and stdout pipes. That is
why servers cannot survive a Core restart, why a crash is recorded as a normal stop, and why Core
cannot tell a frozen server from a healthy one.

Proposed:

- Start Java detached from Core, and record its process id and start time in the server's folder.
- Read the console by following the server's own log file instead of a pipe.
- Send commands over RCON, which Core turns on for every server with a random password and its own
  port, bound to the local machine only.
- On startup, adopt any server whose recorded process is still alive instead of starting a second
  copy.
- Record how each run ended: stopped, crashed (with exit code), or killed.

This one change is what makes these possible: surviving restarts and updates, crash auto-restart,
freeze detection, player counts, and backups while running.

### 1.3 Auth

Today the App sends each user's full account session token to whichever Core they open. The person
running that Core is a friend, not a trusted party, and can replay the token to act as that user on
the backend. Core also asks the backend to vouch for every token, so it is unusable whenever the
backend is unreachable, even for the owner on their own network.

There are two different tokens, and only one is a problem. Core has its own key, issued by the
backend when the owner pairs it, which it uses to report its address. That stays. The problem is the
user's account session, which the App attaches to every request it sends to a Core.

Proposed: before talking to a Core, the App asks the backend for a short-lived token that is valid
for that one Core and carries just the user's id, name, and avatar. The backend signs it. Core
fetches the backend's public key once and checks the signature itself, with no backend call per
request. This touches `apps/backend`, `packages/api-client`, and Core.

Limit: the App still needs the backend to get a token, so a Core with no internet for longer than
the token's lifetime is still locked. Short backend outages stop mattering; long offline use is not
solved here.

Same pass:

- Stop wiping every collaborator when Core gets a single 401 from the backend.
- Say plainly in the invite screen that an editor can run code on the host machine, because
  uploading a mod is running code.
- Hide the RCON password from viewers.

### 1.4 One content model

Core has two systems that both answer "which mods are on this server": the `mods` table for mods
added on the server, and the source files table for mods from the linked pack. They use different
hashes and separate folder scans and are joined by filename.

Proposed: one read path. Content is the files on disk plus optional metadata, with the origin (pack
or server) recorded per file.

### 1.5 Config and platforms

- Give every setting a default so Core starts with no configuration. Today it refuses to start
  without five environment variables, and the release ships no config file.
- Replace the Unix admin socket with the existing HTTPS API plus a local token file, so the CLI
  works on Windows.
- Build releases for Windows x64, macOS arm64, Linux x64, and Linux arm64. Only Linux x64 is built
  today.
- Start with the user's session on Windows and macOS instead of a full system service.
- Trim the CLI to what the product needs. `uninstall` currently removes nothing.

### 1.6 One backup path

Two backup APIs exist with different locking and can run over each other. Keep the queued one.

Back up the world and a short list of settings files instead of the whole server folder. Today every
backup includes all jars and libraries, and a restore reverts `server.properties`.

## Phase 2: bugs

Confirmed bugs that are not fixed as a side effect of phase 1.

| Bug | Effect |
|---|---|
| Console reader stops on the first non-UTF-8 line | The server freezes while Core shows it as running. Likely on Windows with accented names or chat. Goes away with 1.2. |
| Old Forge (before 1.17) looks for `forge-server.jar` | Every install of 1.16.5, 1.12.2, and similar fails. The installer never writes that name. |
| Forge launch detection reads `run.sh` first, even on Windows | Wrong args file on Windows (suspected, not yet confirmed). |
| Repair wipes a shared install while another server is using it | A running server on the same build can crash. |
| Graceful stop kills after a fixed 30 seconds | Large worlds can be cut off mid-save. |
| Adding a mod ignores mods the pack already provides | A duplicate dependency is installed and the server will not boot. Goes away with 1.4. |
| Pack update overwrites files the owner edited | Config changes made on the server are silently reverted. |
| Pack overrides can replace `server.properties`, `ops.json`, the world | A client export can clobber server-owned files. |
| A failed pack update can stick in "failed" | The owner cannot clear it. |
| Downloads have no timeout | A hung download blocks pack sync forever. |
| Backup download link in the panel cannot authenticate | Backups cannot be downloaded. |
| Datapack buttons always return an error | Listed with controls that never work. |
| Stats stream stops on any disk error | CPU and memory graphs go blank until reconnect. |
| Stable release channel picks the repo's latest release | The installer can fetch a Modrinth App release instead of Core. |
| Beta and alpha update channels never resolve | `update --check` checks nothing. |
| Linux service runs as a user that cannot open the firewall | Core cannot open its own ports on ufw or firewalld. |
| Core's API port is forwarded to the internet with no off switch | Exposure for users who only want LAN play. |

## Phase 3: features

What makes Core feel finished. Grouped by what the user gets.

**Servers that stay up** (needs 1.2)

- Restart a crashed server automatically and show why it died.
- Detect a frozen server and offer to restart it.
- Servers keep running through a Core update.
- Scheduled restarts.

**Performance**

- Tuned Java flags by default, shown and resettable in startup settings.
- Memory suggested from the machine's RAM, with a warning when servers together exceed it.
- Change memory after creation. The API accepts it today but the panel never sends it.
- Sleep when empty: stop a server with nobody online and start it when someone joins.

**Backups** (needs 1.2 and 1.6)

- Backups while the server is running.
- Scheduled backups with retention.
- Check free disk space first and clean up a failed backup.
- Download a world.

**Installs and content**

- Real install progress and download retries.
- Update indicators for mods and the pack. The check is a stub today.
- Hard reset.
- Server and loader updates. Paper is pinned to whichever build was current at install.
- Ask about the Minecraft EULA instead of accepting it silently.

**Knowing what is happening** (needs 1.2)

- Player count and player list.
- Logs written to a file with rotation, reachable from the App.
- A complete activity log, including access changes and sensitive reads.

**Reaching the server**

- Show the connection problem and the single next step in the App, not only in Core's terminal.
- A stable address per server, so a changing home IP does not break saved servers.
- IPv6.

## Testing

Coverage of the database, migrations, CLI, and install paths is close to zero. Add focused tests as
each phase 1 item lands, against an in-memory SQLite database, rather than one large testing pass at
the end.
