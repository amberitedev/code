# Upstream

> For maintainers and agents.

Amberite merges Modrinth's `main` and keeps merging it. Upstream code stays in its upstream file
where it can, so each sync is a real merge rather than a hand port.

## Syncing

The `/upstream-sync` skill runs `node scripts/upstream-sync.ts` on a fresh branch from `main`,
resolves what is left, and opens the pull request. Merge it with a merge commit, never a squash: the
next sync finds its merge base through the second parent.

Nothing merges ahead of time. A CI job once prepared a branch every day with the conflict markers
committed; the branch went stale as soon as `main` moved, and one waiting branch blocked every later
run. The merge takes seconds, so it runs when someone is there to finish it.

## What the merge settles

[`upstream-merge.ts`](../../scripts/upstream-merge.ts) leaves a normal merge in progress, with only
real decisions unmerged. It answers the rest from our tree and from
[`upstream-map.ts`](../../scripts/upstream-map.ts):

- **Deleted stays deleted.** A file Modrinth changed and we do not have is left out, and so is a
  new file inside a folder we deleted. Our tree is the record, so there is no list of ignored
  paths. To cut something of Modrinth's, delete it. To take it back, restore it from
  `modrinth/main`.
- **`MOVED`** paths are renamed in Modrinth's tree and in the merge base before merging. The merge
  then pairs files by path: Modrinth's edits, additions and deletions land at our name however far
  our copy has drifted. Git's similarity guess does this by itself for a single file that still
  looks like Modrinth's, and fails for a rewritten file and for a folder of hash-named files.
- **`PORTED`** paths are code we rewrote. Nothing can merge, so the report lists each change with
  the place that does its job here.
- **`SIDES`** names the files that are generated or wholly ours. When both sides changed one, a
  side wins without a conflict.

The report lists every dropped change, so a move or a rewrite missing from the map shows up there
instead of being lost.

## What merging cannot do

A merge only changes Modrinth's files. Our code has to follow in two ways, and each one fails
loudly:

- **Code that calls Modrinth's code breaks the build.** The App's UI reaches the engine through
  typed Endpoints and may not import Tauri's `invoke`, and
  [`labrinth-compat.ts`](../../packages/api-client/src/contracts/labrinth-compat.ts) stops compiling
  when a backend shape no longer fits the labrinth type Modrinth's pages read. A sync that needs
  such a change does not pass CI without it.
- **Code we rewrote shows nothing.** When Modrinth fixes a bug in a file we replaced, our copy
  keeps the bug and every check stays green. So the merge commit lists each such change under "To
  port:", and it stays owed until a commit trailer settles it
  ([`upstream-ports.ts`](../../scripts/upstream-ports.ts)). `node scripts/upstream-sync.ts --ports`
  fails while one has no trailer, CI runs it on every sync pull request, and a change put off comes
  back in the report of every later sync. The record is in commit messages, so it cannot drift from
  the history it describes.

## Porting rules

The sync merges upstream's code; these rules say where an upstream change goes when merging alone
leaves it unreachable or in the wrong place.

- An `api/` function or `#[tauri::command]` with a frontend helper that calls `invoke` becomes an
  Endpoint declared next to its types in the engine and a handler that takes the `Engine`. When its
  body or answer carries a host filesystem path, it becomes an App Endpoint instead (a Core serves
  the engine's `shared` routes to remote users). Only the helper's body changes, to call
  `client.local`; its name and signature stay upstream's.
- A new engine event becomes a variant of the engine's `Event`, and a new desktop event a variant
  of the App's `AppEvent`. Both reach the UI on the `/events` socket.
- A new plugin call becomes an App route.
- A change to a desktop domain (accounts, skins, friends, Discord, settings and the like) goes to
  that domain's folder, `apps/app/src/<domain>`, and its migrations to the App's migrations.
- A new upstream migration is ported by hand into the fresh engine or App schema.
- Upstream's Archon and Kyros calls in our screens become calls of the Core's or the engine's
  Endpoints with `client.core(coreId)`, on their generated types. Where an upstream shape names a
  world, a Core server's one world has the server's id.
- Upstream's shared tab providers are implemented once over `{ client, instanceId }`, where
  `client` is `client.local` or `client.core(id)`; each host page only wires them.
- Upstream's `archon` and `kyros` modules in api-client are deleted: there is no Modrinth Hosting
  to call, so `client.archon` and `client.kyros` do not exist. Their `types.ts` stay as upstream
  has them, for the upstream screens that still name them, with no fields of ours.
- Upstream's `shared-instances` modules stay as upstream has them, minus the Modrinth Hosting
  business modules below. Our screens use our API.
- There is no Modrinth Hosting. Its business code (billing and purchase, Stripe, Intercom, surveys,
  suspension, staff support tools and their api-client modules) is deleted, and nothing falls back
  to Modrinth's hosting or shared-instances hosts.
- Server roles are cut: a Core answers its owner only. Drop upstream changes to server users,
  member invites and roles; their files are deleted, the access page keeps only the activity log,
  and `packages/ui`'s `server-permissions.ts` keeps upstream's exports but grants everything.
- A change to a `PORTED` path is carried into the file the map names. For `apps/backend`, keep
  labrinth's schemas and contracts.
