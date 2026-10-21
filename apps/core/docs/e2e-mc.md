# End-to-end on `mc` (Linux arm64)

The rewritten Core run for real on a Linux arm64 machine, phase by phase. Each phase has its
findings first, then the dated log of what was run and what it did. Times are UTC.

The machine: SSH host `mc`, Oracle Ubuntu 22.04 aarch64, 4 cores, 23 GB RAM, systemd, ufw on,
rustc 1.99.1. It is a live Minecraft host: a NeoForge 21.1 server runs there as `ubuntu`, outside
systemd, on port 25566 (TCP and UDP, plus mod ports UDP 24454 and 56238). Nothing here touched it;
it ran the whole time under the same pid.

## Phase 1: install, service, servers, updater, CLI

`core-rewrite` at `a2dc27f` (steps 1 to 3, 6, and 7 merged). Reachability, IPv6, and port
forwarding (step 8) are not merged and were skipped.

### Findings

Fixed on `core-10-e2e`:

1. **Java could not be installed on Linux arm64, so no server could be created there.** Core asked
   Azul for a ZIP JRE, and Azul publishes none for Linux arm64 (only tar.gz). `copal servers
create` failed with "No Java 21 runtime available for this platform". Linux and macOS now ask
   for the tar.gz package with `crac_supported=false` (without it Azul answers with a CRaC build)
   and unpack the gzipped tar; Windows keeps the ZIP. Java 21 then installed in 7 s and ran
   as `copal-servers`. `src/installs/java.rs`.
2. **Running `copal install` again made every existing world unreadable.** Install chowns
   everything under `servers/` to `copal` and keeps the file modes. Minecraft writes `level.dat`
   as 0600, so after a reinstall (a binary update by hand, or data kept across uninstall) the
   `copal-servers` account cannot open its own world: the server dies at start with
   `AccessDeniedException: ./world/level.dat` and "World files may be corrupted. Shutting down",
   exit 0. Install now also gives the group read and write on everything inside `servers/`.
   Verified on mc: a world with a 0600 `level.dat` owned by `copal-servers` came out 0660
   `copal:copal-servers` and the server started. `src/daemon/folders.rs`, with a test.
3. **Every CLI error printed color-eyre's source location and backtrace hint** (`Location:
apps/core/src/cli/client.rs:97`, `Backtrace omitted. Run with RUST_BACKTRACE=1`). Errors now
   print as one line: `Error: there is no server nope. See copal servers list`. `src/main.rs`.
4. **An account other than the owner got only "Broken pipe (os error 32)".** Core drops such
   connections without a word, which is right, but the CLI did not say why. It now says "Core
   closed the connection without answering. Only the account that installed Copal may use the
   copal CLI". `src/cli/client.rs`.

Found here, decided by the owner, and fixed on `core-10-e2e`:

5. **New servers could be given a port another program is using.** `records::create` took the
   lowest port in 25565-25664 that no Copal server had, without checking the machine, so the
   second server on mc got 25566, the live NeoForge server's port. Creating a server now also
   skips every port in the range that this machine will not let Core bind: TCP and UDP, on IPv4
   and (where the machine has it) IPv6, because Minecraft listens on all of them.
   `src/servers/records.rs`, with tests.
6. **A failed create left a server the CLI could not fix or remove.** The record stayed with
   nothing installed, and the CLI had no delete (the local API's `DELETE /local/servers/{id}`
   worked with curl). There is now `copal servers delete <server>`, and a failed `servers create`
   exits non-zero with "Created server <id>, but installing Minecraft <version> failed:
   <error>" and the delete command to run. `src/cli/servers.rs`.
7. **`copal status` offered an update that had just failed, and said it "did not start".** The
   launcher now records how a trial failed (`failed_reason` in `versions/state.json`: never
   started, exited before it answered, or no answer within a minute; older launchers leave it
   out). `/update` reports it, `update_available` is false for the release that failed, and
   `copal update apply` (and auto-update) refuse it unless the owner passes `--retry`. Status
   says "Copal 0.3.0 was tried and rolled back: it started, but did not answer within a minute",
   and how to retry. `src/update/`, `src/api/update.rs`, `src/cli/machine.rs`.
8. **A killed run showed as "stopped".** Core recorded it correctly; the Core-only
   `power_state` now has `killed` too, and `servers list` shows it. `src/api/servers.rs`,
   `packages/api-client` types.
9. **`sudo copal ...` is refused like any other account.** Kept, as the plan says. When the
   refused CLI runs under sudo (`SUDO_USER` set), it now says to run it again without sudo, as
   the account that installed Copal. `src/cli/client.rs`.
10. **When Core itself crashes, the whole service restarts.** A SIGKILLed `copal core` makes the
    launcher exit, and systemd starts it again after `RestartSec=5`; Core was back and had
    adopted the server 19 s later. As designed; left as is.
11. **The installed program was mode 0775**, copied with the build's mode. Install now sets 0755. `src/daemon/install.rs`.

Everything else in the phase worked as the plan says. The details are in the log below.

- **Install:** both accounts are created without a password or shell, and the unit has
  `KillMode=process`. Folder ownership matches the data folder table, sudoers holds one line,
  and ufw gets Core's port plus 25565:25664/tcp.
- **Isolation:** the socket refuses every other account, and `copal-servers` cannot read `db/`,
  `runs/`, `backups/`, `versions/`, or `sockets/`, nor open a run socket.
- **Servers:** stop takes 3.5 s and kill 0.5 s. Adoption works after `systemctl restart` and
  after a SIGKILL of Core.
- **Updater:** an update with a server running finished in 1.1 s, the health marker landed
  within 1 s, and the server kept its pid through four Core swaps. A manual rollback restored
  the database. A bad version that had migrated was rolled back automatically after 60 s with
  the database restored, and install refuses to downgrade without `--allow-downgrade`.
- **Uninstall:** it removes everything, and ufw is left exactly as before.
- **Build:** 4 min 8 s first, about 2 min after a change, with no warnings on arm64. All 110 copal
  tests pass on arm64.

### Log

#### 2026-10-21

**Baseline (04:54).** `ss -ltnup` as root: TCP 22, 80, 111, 3306, 6379 (local), 25566 (NeoForge);
UDP 53, 68, 111, 24454, 25566, 56238. `ufw status verbose` saved to `ufw-before.txt`: active,
deny incoming, allows 22, 80, 443, 3306, 8080, 2022, 25565/tcp+udp, 25566/tcp+udp. No `copal`
accounts or unit. 24 GB free.

**Build (04:55-04:59).** `git clone --depth 1 -b core-rewrite` into `/home/ubuntu/copal-e2e/src`
(8 s, `a2dc27f`). `cargo build --release -p copal` under nohup: finished in 3 min 56 s (real 4 min
8 s, user 7 min 47 s), 0 warnings. Binary: 9.4 MB, ELF aarch64 PIE, links only libc, libm, and
libgcc_s.

**Install (05:00).** `sudo ./copal-0.1.0 install`:

```
Program: /usr/local/lib/copal/copal
Accounts: copal and copal-servers, without interactive sign-in
Owner: 1001 may use the copal CLI
sudoers: copal may run /usr/local/lib/copal/run-as-servers as copal-servers, nothing else
Data folder: /var/lib/copal
Firewall: ufw: allowed TCP 16662
Firewall: ufw: allowed TCP 25565:25664
Service: systemd unit copal.service enabled and started
```

- `id`: `copal` 996 (also in group `copal-servers`), `copal-servers` 997. `getent passwd`: home
  `/nonexistent`, shell `/usr/sbin/nologin`; `getent shadow`: password `!` for both.
- `systemctl status copal`: active, `copal run` (launcher) with child `copal core`. Unit: `User=
copal`, `KillMode=process`, `Restart=always`, `RestartSec=5`, `UMask=0027`.
- `ls -la /var/lib/copal`: root 755 `copal:copal`; `db backups versions sockets runs logs` 700
  `copal:copal`; `servers meta java` 2750 `copal:copal-servers`; `copal.sock` 0666 (access is
  checked by peer uid). `db/`: `copal.db` 0640, `install.toml` and `tls-key.der` 0600.
- `/etc/sudoers.d/copal`: `copal ALL=(copal-servers) NOPASSWD: /usr/local/lib/copal/run-as-servers`.
- `ufw status`: `16662/tcp` and `25565:25664/tcp` ALLOW (v4 and v6), commented "Copal".
- `ss`: Core listens on `0.0.0.0:16662` (IPv4 only; IPv6 is step 8).

**Owner socket (05:01).** As `ubuntu`: `copal status` → "Core 0.1.0 is running", no servers;
`copal logs -n 20` → Core's start lines; `copal eula` → not accepted; `copal eula --accept` →
"accepted on 2026-10-21 05:01"; `copal --json status` → health, update (`target: linux-arm64`,
channel stable, auto on), servers. As `copal-servers` and `nobody`: `copal status` fails with a raw
"Broken pipe" (finding 4); a raw Python connect to `copal.sock` as `copal-servers` gets
"Connection reset by peer".

**Restart (05:01).** `sudo systemctl restart copal`: back in under 3 s, `copal status` answers.
`versions/` holds only `launcher.lock`: no health marker, as expected. The launcher runs its own
binary until the first update (`versions.rs`); markers are checked in the updater section.

**Create (05:02).** `copal servers create alpha --version 1.21.1` → `Error: could not install Java
21: No Java 21 runtime available for this platform` (finding 1). The server stayed in the list
with nothing installed (finding 6). The Azul query with `archive_type=zip` returns `[]` for Linux
aarch64. With `tar.gz` it returns a CRaC build unless `crac_supported=false` is given; with both,
Java 8, 17, 21, and 25 all exist for Linux aarch64.

**Java fix (05:03-05:05).** Patched `java.rs` (tar.gz on Linux and macOS), rebuilt on mc (2 min 3
s, no warnings), `sudo ./copal-0.1.0-javafix install` (reinstall over the running service: fine),
then retried the install with `curl --unix-socket /var/lib/copal/copal.sock -X POST
http://copal/modrinth/v0/servers/<id>/reinstall?hard=false -d
'{"loader":"vanilla","game_version":"1.21.1"}'` → 204 in 6.7 s. `java/21/zulu21.52.203-ca-
jre21.0.12.1-linux_aarch64`; `sudo -u copal-servers .../bin/java -version` → OpenJDK 21.0.12.1
Zulu. `meta/` holds `versions/1.21.1` and shared libraries, all `copal:copal-servers` 2750.

**Start (05:05).** `copal servers start alpha` → "alpha is running" (0.6 s). Processes: `copal
run` (copal) → `copal core` (copal) → `copal shim --data-dir /var/lib/copal <id>` (copal) → `sudo
-n -u copal-servers -- /usr/local/lib/copal/run-as-servers .../java -Xms512M -Xmx2048M ...` →
`java` as `copal-servers` (uid, gid, and groups all 997). "Done (9.577s)". Java listens on
`*:25565`. `runs/<id>/`: `console.log launch.json lock run.json shim.log`, 0640 `copal`;
`sockets/<id>.sock` 0600 `copal`.

**Isolation (05:06), as `copal-servers`.**

| Attempt                                                                                       | Result            |
| --------------------------------------------------------------------------------------------- | ----------------- |
| `cat db/copal.db`, `ls db/`                                                                   | denied            |
| `ls backups/`, `ls versions/`, `ls runs/`, `ls sockets/`                                      | denied            |
| `cat runs/<id>/console.log`                                                                   | denied            |
| connect `sockets/<id>.sock` (its own server's)                                                | denied (EACCES)   |
| connect `copal.sock`                                                                          | reset by Core     |
| `ls meta/`                                                                                    | allowed           |
| `touch meta/x`, `touch java/x`, `touch servers/x`                                             | denied            |
| `touch servers/<id>/probe`                                                                    | allowed           |
| `sudo -n true` as `copal-servers`; `sudo -n -u root` or `-u copal-servers /bin/sh` as `copal` | password required |

**Second server (05:06).** `copal servers create beta --version 1.21.1` → created on **port
25566**, the NeoForge server's port (finding 5). Never started; deleted with `curl -X DELETE
.../local/servers/<id>` → 204.

**Console (05:07).** `copal servers console send alpha list` → "Sent to alpha: list"; the log
shows "There are 0 of a max of 20 players online". `copal servers console attach alpha` with
`list` piped in: the backlog, then the `list` answer.

**Restart Core with the server running (05:07).** `sudo systemctl restart copal`: Java 2197338
and shim 2197336 unchanged; the shim was reparented to pid 1. Core log: "stopping; servers keep
running", then "adopted a running server". `console send alpha "say adopted"` reached the
server. `run.json` records `v: 1`, run id, shim pid, start time, boot id, Java pid.

**SIGKILL Core (05:08).** `kill -9` of `copal core`: the launcher exited and systemd restarted the
service (`NRestarts` went up), and Core adopted the server 19 s later (finding 10). Java pid
unchanged. (An earlier attempt with an unanchored `pgrep -f "copal core"` also matched and killed
my own SSH shell; harmless, and it accounts for one extra restart.)

**Stop (05:08).** `copal servers stop alpha` → "Stopping alpha... alpha is stopped", 3.5 s.
`status.json`: `exit_code 0, ended_by stop`. Core log: `outcome=Stopped exit_code=Some(0)`.

**Force-kill (05:10).** Start (0.1 s), let it run 80 s, then `copal servers kill alpha` → "alpha is
stopped", 0.5 s. `status.json`: `exit_code 137, ended_by kill`; Core log: `outcome=Killed`. The CLI
shows `stopped` (finding 8).

**Crash (05:11).** Start, wait for "Done", `kill -9` Java behind Core's back. `servers list` →
`crashed`; `status.json` `ended_by: exit`, Core log `outcome=Crashed exit_code=Some(137)`. No
crash restart: that is step 8.

**Updater setup (05:12-05:19).** On a local scratch branch on mc (never pushed): the Java fix,
then `PUBLIC_KEY` in `release.rs` swapped for a throwaway key from `minisign -G -W` (minisign 0.12
static aarch64 binary in `copal-e2e/tools`). The swap is needed because release builds trust only
the release key; `COPAL_UPDATE_PUBLIC_KEY` works in debug builds only. Three release builds,
about 2 min each:

- `0.1.0`;
- `0.2.0`, the version bumped;
- `0.3.0`, deliberately bad: it adds migration `002_e2e_bad.sql` (`CREATE TABLE e2e_bad`) and its
  `Updater::answered` returns at once, so it migrates and serves but never writes its health
  marker.

`publish.sh <version>` copies the binary to `www/copal-v<version>/copal-linux-arm64`, writes
`copal-manifest.json` the way `copal-release.yml` does, signs it with `minisign -S`, checks it with
`minisign -V`, and rewrites `www/releases.json` in GitHub's releases shape.
`python3 -m http.server 18080 --bind 127.0.0.1` serves it, and `config.toml` sets `update_url =
"http://127.0.0.1:18080/releases.json"`.

**Install the test-key 0.1.0 (05:19).** `sudo rel/copal-0.1.0 install`, then `copal update auto off`
(so the 5-minute first check could not race the test). `publish.sh 0.2.0`; `copal update check` →
"Copal 0.2.0 is available (this is 0.1.0): copal update apply".

**The reinstall broke the world (05:22).** `copal servers start alpha` → "alpha is running", but
Java exited 0 seven seconds later: `AccessDeniedException: ./world/level.dat` and
`./world/level.dat_old`, "World files may be corrupted. Shutting down". The reinstall had chowned
`world/level.dat` (0600, written by Java as `copal-servers`) to `copal` (finding 2). For the
moment I repaired it by hand with `chmod -R g+rw servers/<id>`; the server then started ("Done
(3.033s)").

**Update 0.1.0 → 0.2.0 with the server running (05:23, re-checked 05:25).** `copal update apply` →
"Core 0.2.0 is running" in 1.1 s. `versions/0.2.0/healthy` within 1 s; `versions/0.1.0/copal`
kept for rollback; `state.json` `active 0.2.0, previous 0.1.0, trial null`. The launcher log says
"trying the new version" and then "the new version is healthy" 0.26 s later. Core runs from
`/var/lib/copal/versions/0.2.0/copal core` under the original launcher. The server survived
(same Java pid; this is the 05:25 run, after the world repair) and took console commands.

**Manual rollback (05:25).** Under 0.2.0, `copal update channel beta` (a database change after the
update). `copal update rollback` → "Core 0.1.0 is running". `copal update channel` → `stable`, so
the database from before the update was put back. `state.json` `active 0.1.0, previous null`.
Launcher: "rolled back with the database from before the update". Java 2200538 and shim
2200536 unchanged; the server was adopted.

**Bad update 0.2.0 → 0.3.0 (05:25-05:26).** `copal update apply` (back to 0.2.0, healthy in 0.26 s).
`publish.sh 0.3.0`, `copal update apply` → "Core 0.3.0 is running". During the trial `state.json`
had `trial.database_saved: true` and the live database held migrations `[1, 2]` and the `e2e_bad`
table. At 05:26:48, exactly 60 s later, the launcher logged "the new version did not answer within
a minute; going back to the previous one". `state.json`: `active 0.2.0, failed 0.3.0`; the
database had migrations `[1]` and no `e2e_bad` (read with Python's `sqlite3` in read-only mode).
0.2.0 adopted the server (Java 2200538, the same since 05:25). `copal status` printed both "Copal
0.3.0 is available" and "The update to 0.3.0 did not start" (finding 7).

**Reinstall fix on the real machine (05:27-05:32).** Built `core-10-e2e`'s changes on mc
(release 1 min 59 s, no warnings). `cargo test -p copal folders`: 2 passed, including the new
test. `cargo test -p copal`: 110 passed, 1 ignored. Stopped `alpha`; `world/level.dat` was
`-rw------- 997:997` again (rewritten by Java). `sudo rel/copal-0.1.0-fixes install` refused:
"Core has updated itself to 0.2.0, newer than this copal (0.1.0) ... pass --allow-downgrade",
which is the install downgrade guard working. With `--allow-downgrade`: `level.dat` is now
`-rw-rw---- 996:997`; `copal servers start alpha` → "Done (3.048s)". New CLI errors: `sudo -u
nobody copal status` and `sudo copal status` → "Error: Core closed the connection without
answering. Only the account that installed Copal may use the copal CLI: ... Broken pipe" (finding
9); `copal servers start nope` → "Error: there is no server nope. See copal servers list".

**Uninstall (05:32).** `copal servers stop alpha`, then `sudo copal uninstall --remove-data`:

```
Service: removed
Firewall: ufw: removed TCP 16662
Firewall: ufw: removed TCP 25565:25664
Accounts: removed copal and copal-servers
Data folder: removed /var/lib/copal
```

`id copal copal-servers`: no such users. Gone: `copal.service`, `/etc/sudoers.d/copal`,
`/usr/local/lib/copal`, `/var/lib/copal`. No processes left under uid 996 or 997. `ufw status
verbose` and `ufw show added` are identical to before install. Listening sockets are identical to
the baseline once the release server was stopped. The NeoForge server is still on pid 1824701.
**Fixes 5 to 11 on mc (12:02).** Release build of `core-10-e2e` at `c2f8c4d` (2 min 1 s), clippy
clean, `cargo test -p copal`: 114 passed. `sudo rel/copal-pr install`; `stat` on
`/usr/local/lib/copal/copal` → `755 root:root`.

- `copal eula --accept`, run straight after install, failed with "Core is not running ... No
  such file or directory": the service had started but not yet opened its socket. A second later
  it answered. The error also printed its cause twice, which is fixed in `src/cli/client.rs`.
- `sudo copal logs` → "Only the account that installed Copal may use the copal CLI, and not
  through sudo: run it again without sudo, as that account (was it ubuntu?)".
- `copal servers create one` → port 25565. `create two` → **port 25567**: 25566, the NeoForge
  server's, was skipped.
- `copal servers create broken --version 9.9.9` → exit 1 with "Created server <id>, but
  installing Minecraft 9.9.9 failed: meta: unknown game version: 9.9.9: unknown game version:
  9.9.9" and "Delete it with: copal servers delete <id>". (The doubled cause comes from
  Core's own error text for the install, not the CLI.)
- `copal servers delete broken` → "Deleted broken (<id>)", exit 0; `delete two` the same.
- `copal servers start one`, then `kill one` after 20 s → "one is killed"; `servers list` shows
  `killed`, and `--json` gives `"power_state": "killed"`.
- `sudo copal uninstall --remove-data`: accounts, folders, and program gone; `ufw status
verbose` and the listening sockets are identical to the baseline; NeoForge still on pid 1824701.

The update fixes (7) are covered by tests rather than a second signed-release run: the launcher
records the reason, `/update` reports it, and a failed release is neither offered nor applied
without `retry`.

**Left on mc for phase 2:** `/home/ubuntu/copal-e2e` (clone, target dir, minisign and the
throwaway key, publish script; 2.6 GB) and `~/.cargo/registry` (302 MB). They are removed at the
end of phase 2.

## Phase 2: loaders, content, crash restart, tickets, backups

`core-rewrite` at `c60b069` (steps 1 to 4 and 6 to 8, and the App's switch to tickets), with
step 5 (backups) added when it merged. Fixes are on `core-12-e2e-2`.

### Findings

Fixed on `core-12-e2e-2`:

1. **`Cargo.lock` did not match the manifests**, so every `--locked` build failed, the release
   workflow's included (`winx` wanted `bitflags 2.13.2`). One line.
2. **A file Core may not read was called "not a plain file".** `open_plain` mapped every failed
   open of an existing entry to that message, which hid finding 5. It now says "Core may not read
   world/level.dat_old". `src/content/folder.rs`, with a test.
3. **`copal backups schedule` used another shape than Core.** The CLI had its own
   `every_hours` struct, while the API answers and takes `interval_hours` (0 is off). It always
   printed "no scheduled backups", and setting one failed with 422. It now uses
   `api::backups::BackupSchedule`. `src/cli/backups.rs`.
4. **`copal backups restore --help` promised the current world is backed up first.** It is
   replaced and not kept (finding 6). The help says so now.

Open, for a decision:

5. **Core cannot read the world files Java writes, so backups and pack installs fail on Linux.**
   This is the big one. Minecraft writes `world/level.dat` and `level.dat_old` as 0600, owned by
   `copal-servers`; `copal` reads servers' files through the group, which has no access to them.
   Once a server has started:
   - every hot backup fails (`save-all flush` rewrites `level.dat` right before the copy), and a
     cold one fails too, in `Backups::copy` reading the world (`src/backups/mod.rs:1042`);
   - every pack install and pack update fails, because each makes a pre-update backup first
     (`src/content/install.rs:98`, `:241`; `Backups::pre_update`, `src/backups/mod.rs:357`).

   The API answers 202 and the CLI exits 0; the failure is only in Core's log, the backup's
   `error` status, and the pack's `error` field. Java sets 0600 explicitly, so a umask or a default
   ACL cannot fix it. Suggestion: read the world through the account that owns it. Core may
   already run any program as `copal-servers` through `/usr/local/lib/copal/run-as-servers`;
   before copying, it could run `chmod -R g+rX` on the world folders that way (cheap, and nothing
   new can be read: the world belongs to the mod's account anyway), or stream the files out with
   `tar` run the same way. Giving the service `CAP_DAC_READ_SEARCH` would also work but lets Core
   read every file on the machine. Windows should not be affected (the folder's inherited ACL applies to what Java writes), but this run did not check it.

6. **A restore throws away the world it replaces.** On success the moved-aside world is deleted
   (`src/backups/mod.rs:1181`). The plan does not promise a pre-restore backup, but a mistaken
   restore cannot be undone. Either keep it as a backup, or make the App say it plainly.
7. **A pack update can change the Minecraft version, and an older pack can go over a newer
   world.** The update takes the project's newest version for any game version
   (`src/api/content.rs:1106`, `versions(&project_id, &[], None)`): 26.2.0-7.35 went to
   26.3.0-7.36, so Minecraft 26.2 became 26.3. Installing TVE for 26.2 over the 26.3 world was
   accepted without a warning; Minecraft 26.2 loaded it.
8. **PR 44 edited the shipped `001_init.sql`.** A Core installed from earlier `core-rewrite`
   failed at every start ("migration 1 was previously applied but has been modified"), restarting
   every 5 s, with the reason only in the journal. Harmless before release; from here on, a
   schema change needs a new migration file.
9. **Failed background work is hard to see from the CLI.** `copal backups create` returns at
   once; a failed backup shows `error` in `backups list` with no reason. A failed pack install is
   the same through the API.
10. **The pairing code of an installed Core is only in the journal.** Core prints it with
    `println!` (`src/access/pairing.rs:63`); with the service that goes to `journalctl -u copal`,
    not to `copal logs` or `copal status`.
11. **Small ones:** NeoForge's installer leaves `tmp/neoforge_installer…/` and its log in the
    server folder. A restore does not bring back empty folders (`datapacks/`, `players/`). On a
    machine whose router has no UPnP, Core's log repeats the multi-line reachability advice on
    every check.

What worked:

- **Loaders:** Fabric, Paper, and NeoForge each installed and reached "Done" on Minecraft 26.3.
  NeoForge's installer ran as `copal-servers`.
- **Pack update:** an edited config was left alone and stayed listed as edited.
- **Crash restart:** a crashed server restarted twice, then stopped at the cap with its crash
  report.
- **Pairing and tickets against the real backend:** pairing, the ticket-to-session exchange over
  TLS with the fingerprint matching the backend's, `ticket_required` and `ticket_reused`, and the
  state report with LAN and public addresses.
- **Restore:** a restored world matched its backup byte for byte.
- **Cleanup:** uninstall left ufw and ports exactly as found.

### Log

#### 2026-10-21

**Baseline (12:42).** ufw identical to phase 1's `ufw-before.txt`; the same listening ports; the
NeoForge server still pid 1824701. The `core-rewrite` clone on mc moved to `c60b069`; `cargo build
--release -p copal`: 2 min 50 s, no warnings. `sudo copal install`: same output as phase 1;
`copal eula --accept`.

**Loaders (12:46-12:48), on Minecraft 26.3 (the newest release).** Created with `POST
/local/servers`, then `POST /v1/servers/{id}/worlds/{id}/content` with `{"content_variant":
"bare", "loader": ..., "version": "", "game_version": "26.3"}` (empty version = newest).

| Server   | Install        | Start to "Done"         | Stop  | Port  |
| -------- | -------------- | ----------------------- | ----- | ----- |
| Fabric   | 0.19.5, 10 s   | 16 s ("Done (3.592s)")  | 1.0 s | 25565 |
| Paper    | build 210, 2 s | 23 s ("Done (16.431s)") | 1.5 s | 25567 |
| NeoForge | 26.3.0.95, 9 s | 17 s ("Done (3.035s)")  | 3.0 s | 25568 |

Java 25 was installed once (tar.gz) and shared. Every stop recorded `ended_by: stop`, exit 0.
Paper skipped 25566, the NeoForge server's port. NeoForge's installer ran as `copal-servers`:
`run.sh`, `user_jvm_args.txt`, `libraries/` and the installer log are owned by `copal-servers`.
It leaves `tmp/neoforge_installer…/` and `neoforge-26.3.0.95-installer.jar.log` in the server
folder.

**Pack install (12:48-12:49).** The Vanilla Experience (`EnFnYkUj`, 1.3 MB, 132 mods, server side
required) at `26.2.0-7.35` (`tYfSFTpy`) onto the Fabric server, whose world was made by 26.3:
`{"content_variant": "modpack", "spec": {"platform": "modrinth", "project_id": "EnFnYkUj",
"version_id": "tYfSFTpy"}, "soft_override": true}` → 202. It failed in 6 s, in Core's log only:
`install failed … error=level.dat_old is not a plain file`; the pack's `error` field in `GET
.../addons` said the same. Cause: before a pack install Core copies the world aside
(`backups::pre_update`), and Minecraft writes `world/level.dat` and `level.dat_old` as 0600 owned
by `copal-servers`, which `copal` cannot read. The failed copy left `backups/<id>/pre-update-1`
with an empty `world/`.

To go on, as admin: `sudo -u copal-servers chmod -R g+rX servers/<id>/world`. The install then
took 6 s: TVE 26.2.0-7.35, Fabric 0.19.3, Minecraft 26.2, 132 addons. Core did not warn that 26.2
is older than the world. The server started and reached "Done" in 16 s; Minecraft 26.2 loaded the
26.3 world without a word.

**Edit and pack update (12:50).** Through the files API, as the App does: `GET
/servers/{id}/modrinth/v0/fs/download?path=/config/areas.json5`, a comment line appended, `PUT
.../fs/update?path=/config/areas.json5` → 204. `GET /servers/{id}/content/edited` → `[{"path":
"config/areas.json5", "deleted": false, "pending": null}]`. Stopped the server; `GET
.../content/modpack/update` → `has_update: Znom14dQ` (26.3.0-7.36). `POST` it → 202, and it
failed the same way (`level.dat_old`, which Minecraft had rewritten as 0600). After the same
`chmod` by hand it took 6 s: TVE 26.3.0-7.36, Minecraft 26.3, Fabric 0.19.5, 131 addons. The
edited file kept its SHA-1 (`dc532bf5…`) and is still listed as edited, with no pending decision
(the pack did not change it). `backups/<id>/` keeps `pre-update-3` and `pre-update-4`.

**Crash restart (12:51-12:52).** Started the Fabric server (crash restart on by default), then
`kill -9` its Java three times, each once it was up:

- kill 1 (12:51:25): Core started it again 6 s later;
- kill 2 (12:51:47): again after 6 s;
- kill 3 (12:52:09): left stopped. `servers list`: `crashed`; `servers info`: "crashed 3 times
  in 10 minutes, so Core left it stopped", last crash exit code 137 with its console lines; Core
  log `outcome=Crashed exit_code=Some(137)` and the warning.

**Pairing and tickets (12:53-12:57).** On this PC, `vp run dev:backend` (Worker on
`127.0.0.1:8941`, its own ticket key and `keys.json`), then `ssh -N -R 8787:127.0.0.1:8941 mc`.
On mc, `config.toml` got `backend_url = "http://127.0.0.1:8787"` and Core was restarted.

- The pairing code went to stdout only, so with the service it is in `journalctl -u copal`
  ("enter MZUX-D576"), not in `copal logs` or `copal status`.
- `POST /_dev/session {"username": "scenario_1"}` with the dev secret → an `mra_` session.
  `POST /cores/approve {"code": "MZUX-D576"}` → 200 `{id: c5c82ae6-…, name: minecraftservers}`.
  Core: "Signed in as scenario_1", "connected to the backend".
- `GET /v3/cores` → address `https://10.0.0.225:16662`, public address
  `https://129.159.130.127:16662` (found by Core itself), version 0.1.0, `owned: true`,
  `cert_fingerprint 7583060e…`, the SHA-256 of the certificate `openssl s_client` got from
  `127.0.0.1:16662`.
- `POST /cores/{id}/ticket` → a JWT; Core's `POST /sessions {"ticket", "device_name": "e2e-mc"}`
  over TLS → a session for scenario_1, 30 days. The same ticket again → `ticket_reused`.
- `GET /v1/servers` with the session → the three servers.
- `DELETE /local/servers/{deltest}` with the session and no ticket → 401 `ticket_required`; with
  a fresh ticket in `X-Amberite-Ticket` → 204; that ticket again → 401 `ticket_reused`.
- No session → 401 `unauthorized`; an `Origin` header → 403 "Browsers cannot call a Core".
- `copal access sessions` lists the session (scenario_1, e2e-mc).

Then the tunnel and the backend were stopped, and the dev secret and session files on mc deleted.
(One `POST /local/servers` over the socket answered nothing once, just before; it worked on the
next try and did not happen again.)

**Backups and restore (13:03-13:08), after PR 44 merged and before the restore-help fix.** Merged `core-rewrite` (`f2dabeb`) into
`core-12-e2e-2` and built it on mc with `--locked` (2 min 46 s). `sudo copal install` over the
existing data: Core then failed at every start, restarting every 5 s, with `migration 1 was
previously applied but has been modified` in the journal. PR 44 changed `001_init.sql`. So
`uninstall --remove-data` and a fresh install; a new Vanilla 26.3 server `bk`.

- **Hot backup, server running:** `copal backups create bk` printed "Backing up bk (<id>)", exit 0. The console showed "Automatic saving is now disabled", "Saved the game", "Automatic saving is
  now enabled"; then `backups list` showed `error`, with the reason only in Core's log: `Core may
not read world/level.dat_old`. `save-all flush` rewrites `level.dat` as 0600 just before the
  copy, so no hand-made `chmod` can help a hot backup.
- **Cold backup:** stopped the server, made the world group-readable by hand as before, recorded
  the SHA-256 of its 25 files, `copal backups create bk` → `done` in about a second, a 2.2 MB zip
  in `backups/<id>/`, 0640 `copal`. The server's first start afterwards also made a "Scheduled
  backup" (daily by default; it was due).
- **Change, then restore:** started the server, `time set 18000`, `save-all flush`, stopped, and
  added `world/e2e-marker` as `copal-servers`: 8 files differed from the backup. `chmod` by hand
  again, `copal backups restore bk 8800c191` → "Restoring … on bk", done within a second. Every
  one of the 25 files matched the backup's SHA-256, and the marker was gone. Restored files are
  0660 `copal:copal-servers`. The empty `datapacks/` and `players/` folders did not come back
  (Minecraft makes them again). The replaced world was not kept anywhere, although `copal
backups restore --help` said it is backed up first.
- **Schedule:** `GET /servers/{id}/backups/schedule` → `{"interval_hours": 24, "keep": 7}`, but
  `copal backups schedule bk` said "no scheduled backups", and `--every 6` failed with 422
  `missing field interval_hours`: the CLI used its own struct with `every_hours`. Fixed; rebuilt
  and reinstalled: `schedule bk` → "every 24 hours, keeping the last 7"; `--every 6 --keep 3`,
  `--off`, and back to 24/7 each answered as set. The installed binary is 0755.

**Cleanup (13:09).** `copal servers list` showed only `bk` stopped. `sudo copal uninstall
--remove-data`: service, firewall rules, accounts, data folder removed; no `copal` or
`copal-servers` processes; `/etc/sudoers.d/copal`, `/usr/local/lib/copal`, and the unit gone.
`ufw status verbose` and the listening sockets are identical to the very first baseline. The
NeoForge server is still pid 1824701. Deleted `/home/ubuntu/copal-e2e` (clone, target dir,
minisign, logs; 5.3 GB) and `~/.cargo` (491 MB). Not touched: `~/.rustup` (1.8 GB of toolchains),
and the `. "$HOME/.cargo/env"` lines rustup added to `~/.bashrc` and `~/.profile`, which now make
a login shell print `.profile: line 28: /home/ubuntu/.cargo/env: No such file or directory`.

## Phase 3: the phase 2 fixes, checked

`core-rewrite` at `9cbe554` (PR 54, the fixes for phase 2's findings 5 to 11). One short pass.

### Findings

1. **Fixed on `core-17-e2e-3`: the App's pack update got 400.** PR 54 gave `POST
.../content/modpack/update` an optional `Json` body for `force`. axum refuses an empty body
   sent as `Content-Type: application/json`, and `packages/api-client` sends that header on
   every request (`abstract-client.ts`, `buildDefaultHeaders`), so `updateModpack()` answered
   "Failed to parse the request body as JSON: EOF". The handler now reads the body itself: empty
   means no options. `src/api/content.rs`, with a test.
2. **Every finding PR 54 fixed held on the real machine:**
   - hot and cold backups of a server whose world has 0600 files;
   - a "Before restore" backup that holds the replaced world;
   - pack install and update with their "Before update" backups;
   - a Minecraft move reported, and an older pack refused without `force`;
   - reasons in `copal servers info`;
   - migrations `001` and `002` reopening an existing database.
3. **Small, open:**
   - `copal backups list` labels every automated backup "(scheduled)", including "Before
     restore" and "Before update".
   - `copal status` showed the pairing code only about 55 s after Core started; until then it
     says Core is still getting a code.
   - sudo logs each server start to the journal with Java's full command line, many lines of
     class path; a `Defaults:copal !syslog` line in the sudoers file would quiet it.

### Log

#### 2026-10-21

**Build and install (14:17-14:21).** rustup again (minimal profile, `--no-modify-path`), a fresh
`--depth 1` clone of `core-rewrite` (`9cbe554`), `cargo build --release --locked -p copal`: 4 min
48 s, no warnings. ufw and the listening ports saved as this pass's baseline. `sudo copal install`
now ends with "Run copal status for the code that makes this Core yours in Amberite."

**Fabric server (14:22).** Fabric 0.19.5 on Minecraft 26.2, "Done" 16 s after start; `time set
6000`. Its `world/level.dat` and `level.dat_old` are 0600 `copal-servers`, as in phase 2.

**Hot backup while running (14:22).** `copal backups create fab --name hot` → `done` within a
second. The console: "Automatic saving is now disabled", "Saved the game", "Automatic saving is
now enabled". `level.dat` is now 0640, readable by the group.

**Cold backup, change, restore (14:23).** Stopped, recorded the SHA-256 of the world's 30 files,
`copal backups create fab --name cold` → `done`. Started, `time set 18000`, `save-all flush`,
stopped, added `world/e2e-marker`: 8 files differed. `copal backups restore fab <cold>` → a
"Before restore" backup (automated) first, then the restore. Every one of the 30 files matched
the cold backup, the marker was gone, and the empty `datapacks/` and `players/` folders came back.
The "Before restore" zip holds `world/e2e-marker`, so it is the world that was replaced.

**Pack install and update (14:24-14:25).** TVE 26.2.0-7.35 on the 26.2 server → done in 8 s, 132
addons, after a "Before update" backup. `POST .../content/modpack/update` with
`content-type: application/json` and no body → **400** "Failed to parse the request body as JSON:
EOF while parsing a value" (finding 1). The same request without the header → 202, done in 8 s:
TVE 26.3.0-7.36, Fabric 0.19.5, Minecraft 26.3, 131 addons, another "Before update" backup. Core
logged "the pack moved the server to another Minecraft from=26.2 to=26.3". The server started and
reached "Done" in 16 s.

**Failed installs (14:25).** An unknown version id (`ZZZZZZZZ`) → 202, then `copal servers info
fab` printed "last install failed: https://api.modrinth.com/v2/version/ZZZZZZZZ answered 404 Not
Found". No backup failed, so `copal backups list` had no reason to show; it listed the backups.
TVE 26.2.0-7.35 over the now 26.3 world without `force` → `servers info`: "This pack is for
Minecraft 26.2, older than the 26.3 this server's world is from, and a world does not go back a
version safely. Install it with force to do it anyway." The pack stayed at 26.3.0-7.36.

**Restart and pairing code (14:26).** `vp run dev:backend` on this PC and `ssh -R 8787:…` again,
`backend_url` in `config.toml`, `sudo systemctl restart copal`. Core opened the existing database:
`_sqlx_migrations` holds `1 init` and `2 backup queue`, and the server and its backups were all
there. `copal status` first said "This Core is not paired yet and is still getting a code from
Amberite. copal logs says why."; about 55 s after the start it said "This Core is not paired yet.
Open Amberite, go to Settings > Cores > Add Core, and enter 37JL-NY32." Then the tunnel and the
backend were stopped.

**The fix, on mc.** Built `core-17-e2e-3` (2 min 49 s), `sudo copal install` over the same
data, and the pack update with `content-type: application/json` and no body → 202; the install
finished with no failure in `servers info`.

**Cleanup.** `sudo copal uninstall --remove-data`; no `copal` accounts, processes,
sudoers file, program, unit, or data folder left. ufw and the listening ports are identical to
this pass's baseline; the NeoForge server is still pid 1824701. Deleted `~/copal-e2e-3` (1.2 GB)
and `~/.cargo` (491 MB) again. `~/.rustup` and rustup's lines in `~/.bashrc` and `~/.profile` are
as phase 2 left them.
