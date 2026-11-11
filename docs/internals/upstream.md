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
  wins. The merge runs with `merge.directoryRenames=true`, so a file upstream adds to a renamed
  folder lands under our name.
- `ported`: an upstream path and our file that does its job. Commits that touch it land in the
  summary's `port` bucket. When our tree does not have the path, the merge deletes it, so a file we
  replaced never comes back. A ported path we still have merges as usual and is only reported.

## Porting rules

The sync merges upstream's code; these rules say where an upstream change goes when merging alone
leaves it unreachable or in the wrong place.

- An `api/` function or `#[tauri::command]` with a frontend helper that calls `invoke` becomes an
  Endpoint declared next to its types in the engine and a handler that takes the `Engine`. Only the
  helper's body changes, to call `client.local`; its name and signature stay upstream's.
- A new event becomes an `Event` variant on the socket.
- A new plugin call becomes an App route.
- A change to a desktop domain (accounts, skins, friends, Discord, settings and the like) goes to
  that domain's folder, `apps/app/src/<domain>`, and its migrations to the App's migrations.
- A new upstream migration is ported by hand into the fresh engine or App schema.
- Upstream's Archon calls in our screens go to the engine API.
- Upstream's `archon` and `shared-instances` modules in api-client stay as upstream has them, for
  upstream's own pages. Our screens use our API.
- A `port` commit is carried into the file `ported` names, keeping labrinth's schemas and contracts
  for `apps/backend`.
