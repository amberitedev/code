# Local Hosting integration

The desktop app keeps the inherited Hosting management screens. In development,
the existing Archon and Kyros API-client modules point at Core's `/hosting`
adapter. The adapter calls Core's existing instance, installation, filesystem,
process and backup services. Hosting does not pass through app-lib.

Run `vp run dev 1 2` for two isolated apps, the local account backend, storage
processes and one Core. Read actual ports from `.data/runtime.json`. Open Servers
in either app and use New server to enter the existing content setup flow.

This mode is deliberately local: Core requires a loopback bind and an explicit
development frontend origin. Account pairing and permanent Core ownership are
deferred. Both app scenarios operate the same Core as a local developer.

## Implemented behavior

- Server creation, listing, details, rename and power actions.
- Existing onboarding for a Minecraft loader, a Modrinth modpack or an uploaded
  `.mrpack`. Pack installation also binds its required Minecraft runtime.
- Server properties, Java-version selection, content repair and mod management.
- Filesystem browsing, editing, upload sessions, download, move, copy, deletion,
  ZIP creation and ZIP extraction.
- Authenticated node WebSocket protocol for console, process state and measured
  statistics; panel SSE snapshots and invalidation after changes.
- Real backup archives, download, rename, deletion and restoration. The Hosting
  backup queue stores operation states and history in SQLite. Restore first
  requires a successful safety backup with the name submitted by the client.
  Pending work can be cancelled; physical work cannot be interrupted safely.
  Unfinished work after restart is reconciled or reported as failed for retry.
- Billing and purchase screens and their requests are omitted in local mode.
  No payment or subscription success is fabricated.

Minecraft metadata/libraries and server installations remain shared outside the
individual instance directories. Java detection and management are unchanged.

## Deliberately incomplete capabilities

One world maps to one Core instance. Pairing, account-based server ownership,
friend management access, tunneling, public deployment, DNS, SFTP and additional
port allocations are deferred. Unsupported mutations return errors.

Hard world resets, arbitrary startup commands/JDK vendors, game-version migration
preview/apply, automatic modpack updates, non-mod addon installation and tar
extraction are not implemented by this adapter. Network-byte statistics are not
invented. The adapter is not complete Modrinth Hosting parity.

## Focused verification

`scripts/check-local-hosting.mjs` uses the built API client against a real local
Core. It retains a sample server and records timings in
`.data/hosting-proof.json`. Its optional pack fixture is
`.data/hosting-proof.mrpack`, a real ZIP with a Minecraft dependency and an
override directory/file. This is command-level verification, not a complete UI
or Minecraft gameplay test.

For the manual handoff, check New server and content selection, console start
and stop, file upload/edit/download, backup creation/download/restore and rename
in Settings. Refresh the other app to check that it sees the same Core state.
Neither app should display a purchase wall or request payment setup.

### Verification on 2026-10-04

The real-Core command proof passed ten groups: server creation/details,
rename/properties, file CRUD, upload finalize/cancel, path and world isolation,
queued backup create/download/restore/delete, WebSocket authentication and
statistics, SSE, Minecraft installation, and local mrpack overrides/properties.
After the final rebuild, targeted checks also passed shared-runtime reuse between
vanilla and mrpack setup, restore with the start/restore lock, and a clean node
WebSocket close handshake. Both isolated native apps were running and responding
at handoff. Full UI behavior and Minecraft gameplay remain manual checks.
