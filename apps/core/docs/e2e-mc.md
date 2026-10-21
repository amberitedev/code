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
   and unpack it with `tar`/`flate2`; Windows keeps the ZIP. Java 21 then installed in 7 s and ran
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
- `copal servers create broken --version 9.9.9` → exit 1 with "Created server 41ba8868-…, but
  installing Minecraft 9.9.9 failed: meta: unknown game version: 9.9.9: unknown game version:
  9.9.9" and "Delete it with: copal servers delete 41ba8868-…". (The doubled cause comes from
  Core's own error text for the install, not the CLI.)
- `copal servers delete broken` → "Deleted broken (41ba8868-…)", exit 0; `delete two` the same.
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
