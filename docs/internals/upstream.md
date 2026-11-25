# Upstream

> For maintainers and agents.

Amberite merges Modrinth's `main` and keeps merging it. Upstream code stays in its upstream file
where it can, so each sync is a real merge rather than a hand port.

## The daily sync

`.github/workflows/upstream-sync.yml` runs daily. When Modrinth has new commits and no earlier sync
is waiting, it runs `node scripts/upstream-sync.ts` on a new `sync/upstream/N` branch and pushes it:
one merge commit with both parents, `upstream.json` applied, lockfiles taken from Modrinth and
pruned, and any conflict markers committed as they are. The run's `upstream-sync-N` artifact holds
the script's JSON summary, the new commit titles, the merged Modrinth pull requests, and the diff of
Modrinth's changelog. The `/upstream-sync` skill resolves the branch and opens the pull request.
Merge it with a merge commit, never a squash.

The workflow checks out and pushes with the `UPSTREAM_SYNC_TOKEN` secret, because GitHub never lets
`GITHUB_TOKEN` push changes to `.github/workflows`. It is a fine-grained personal access token for
`amberitedev/code` only, with Contents and Workflows set to read and write. When it expires the
Merge step fails at checkout; replace the secret with a new token.

## upstream.json

Every key is an upstream path, a file or a folder. Upstream's commits name upstream paths, so the
summary buckets commits by them; the script maps them through `renamed` before it touches our tree.

- `ignore`: paths we never take. The merge deletes them every time.
- `renamed`: an upstream path prefix and the prefix it lives under here. The longest matching prefix
  wins. Before merging, the script moves these prefixes to our names in both the merge base and
  upstream ([`upstream-merge.ts`](../../scripts/upstream-merge.ts)), so the merge pairs files by
  path, not by git's similarity guess: upstream's edits, additions and deletions land at our name
  however far our copy has drifted, and nothing comes back at upstream's path. It must list every
  folder we moved.
- `ported`: an upstream path and our file that does its job. Commits that touch it land in the
  summary's `port` bucket. When our tree does not have it under our name, the merge deletes it, so a
  file we replaced never comes back. A ported path we still have merges as usual and is only
  reported.

## Porting rules

The sync merges upstream's code; these rules say where an upstream change goes when merging alone
leaves it unreachable or in the wrong place.

- An `api/` function or `#[tauri::command]` with a frontend helper that calls `invoke` becomes an
  Endpoint declared next to its types in the engine and a handler that takes the `Engine`. When its
  body or answer carries a host filesystem path, it becomes an App Endpoint instead
  ([architecture](architecture.md#one-protocol)). Only the helper's body changes, to call
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
- Upstream's `archon` and `kyros` modules in api-client are under `ignore`: there is no Modrinth
  Hosting to call, so `client.archon` and `client.kyros` do not exist. Their `types.ts` stay as
  upstream has them, for the upstream screens that still name them, with no fields of ours.
- Upstream's `shared-instances` modules stay as upstream has them, minus the Modrinth Hosting
  business modules below. Our screens use our API.
- There is no Modrinth Hosting. Its business code (billing and purchase, Stripe, Intercom, surveys,
  suspension, staff support tools and their api-client modules) goes under `ignore`, and nothing
  falls back to Modrinth's hosting or shared-instances hosts.
- Server roles are cut: a Core answers its owner only. Drop upstream changes to server users,
  member invites and roles; their files are under `ignore`, the access page keeps only the
  activity log, and `packages/ui`'s `server-permissions.ts` keeps upstream's exports but grants
  everything.
- A `port` commit is carried into the file `ported` names, keeping labrinth's schemas and contracts
  for `apps/backend`.
