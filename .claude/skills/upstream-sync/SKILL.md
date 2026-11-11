---
name: upstream-sync
description: Finish the daily Modrinth upstream merge. Resolve the conflicts on the waiting sync/upstream/N branch, keep big new features dormant, port changes to ported paths, and open the PR. Use when the user or a scheduled task runs /upstream-sync.
disable-model-invocation: true
---

# Upstream sync

Each day `.github/workflows/upstream-sync.yml` merges `modrinth/main` into a new
`sync/upstream/N` branch with conflict markers committed as they are (see
`docs/internals/upstream.md`, which also holds the porting rules). This session finishes one such
branch and opens its pull request.
It usually runs unattended in a fresh worktree: decide, do not ask. Never merge anything.

## 1. Find the branch

1. `git fetch origin` and list `origin/sync/upstream/*`.
2. Take the branch that has no pull request in any state (`gh pr list --state all --head <branch>`).
3. If there is none, reply with one line, such as "No upstream sync waiting.", and stop.

## 2. Get the branch and its notes

1. If you are in the primary checkout (`git rev-parse --git-dir` is `.git`), never switch its
   branch: `git worktree add ../upstream-sync-<N> <branch>` and work there. Otherwise
   `git switch <branch>`. Commit nothing on top yet.
2. `gh run download --name upstream-sync-<N> --dir .data/upstream-sync`. It holds:
   - `summary.json`: `base` (the previous merge-base), `conflicts`, `lockfiles`, and the new commits
     in buckets: `port` (touches an `upstream.json` ported path), `ignored` (only touches ignore
     paths), `new` (adds an app, package, page, or route), `overlap` (touches files we changed),
     and `other`.
   - `commits.txt`, `prs.json` (title and body of each merged Modrinth PR), and `changelog.diff`
     (Modrinth's `packages/blog/changelog.ts`).
3. Before running anything, wait for worktree setup as `AGENTS.md` describes.

## 3. Resolve the conflicts

Find every marker with `git grep -n -E '^(<<<<<<<|=======|>>>>>>>)( |$)'`. Rules:

- Modrinth's structure wins. Take their layout, names, and refactors, then re-apply our behavior on
  top. Do not keep our older shape of upstream code to save effort.
- Ads, posthog and analytics, and the MOTD profanity filter stay removed. If upstream adds to or
  moves them, drop that part.
- `upstream.json` ignore paths, and ported paths our tree no longer has, stay deleted. Never
  restore one; if upstream code imports from one, cut the import the same way earlier syncs did.
- A lockfile marked `failed` in `summary.json` failed only because its manifest had markers.
  Regenerate it after the manifests are fixed (`cargo metadata --format-version 1`,
  `pnpm install --lockfile-only`).
- `git log -i --merges --grep='merge modrinth' origin/main` lists earlier syncs; they show how the
  same files were settled before.

## 4. Amend the merge

`git add` the resolved files and `git commit --amend --no-edit`. The merge commit keeps both
parents. Never rebase, squash, or recreate it.

## 5. Check

Run focused checks on what changed: `cargo check -p <crate>` for changed Rust crates, and
`vp run --filter <workspace> typecheck` plus `vp lint <files>` for changed workspaces. A crate this
machine cannot build (missing Java or WebKit) is left to CI; say so in the PR. Fix failures in the
merge commit (amend again), not in a follow-up commit.

## 6. Keep big new features dormant

A big new feature is one that adds a page, flow, or service Amberite has not decided to ship. Merge
all of its code, then hide only its entry points (route, nav item, button) in one small commit
titled `chore(upstream): keep <feature> dormant`, with a comment at each change starting
`// Dormant upstream feature:`. Never revert an upstream commit. Small features and fixes ship as
they are.

## 7. Port

For each `port` commit, read its diff to the `upstream.json` ported paths it touches
(`git show <sha> -- <paths>`) and carry the change into the file each one maps to, following the
porting rules in `docs/internals/upstream.md`. Apply the same rules to any other merged change they
cover. Port small changes in a commit titled `feat(<scope>): port <change> from upstream`, such as
`feat(backend): port user bans from upstream`. List larger ones in the PR instead of porting them.

When a `port` commit renames a ported path upstream, add the new upstream path to `ported` in
`upstream.json`, and check the later commits in this sync that touch it: only the listed name is
reported. If our tree did not have the old path, delete the copy the merge brought in at the new
one, or the next sync treats it as a file we kept. Commit both in
`chore(upstream): track <path> at its new name`, also when the rename came with nothing to port.

## 8. Push and open the PR

1. `git push --force-with-lease`. The amend rewrote the merge the workflow pushed.
2. Open a real PR (not a draft) titled `chore: merge Modrinth upstream (sync N)`. Features come
   first; draw on `commits.txt`, `prs.json`, and `changelog.diff`. Sections, in order:
   1. **New features**: each feature, whether it shipped, was kept dormant, or was skipped, and what
      enabling or porting it would take.
   2. **Ported**: what was carried into our ports, and the larger `port` changes left for later.
   3. **Fixes you get for free**: one line each.
   4. **Ignored N commits**: one line.
   5. **Conflicts**: short, one line per file or group of files.
3. End the body with "Merge with a merge commit. Never squash or rebase: the sync needs both
   parents." and a line naming the model and harness.
4. Reply with the PR link and stop.
