---
name: upstream-sync
description: Merge Modrinth's main into Amberite and open the pull request. Use when the user or a scheduled task runs /upstream-sync.
disable-model-invocation: true
---

# Upstream sync

One run takes Amberite from its last Modrinth merge to Modrinth's current `main`: one merge commit,
the ports it calls for, and a pull request. It often runs unattended: decide, do not ask. A
maintainer merges the pull request.

`scripts/upstream-sync.ts` does the merge and prints a report. Each section of the report says what
to do with it. `docs/internals/upstream.md` says what the merge settles by itself and holds the
porting rules.

## 1. Start the merge

1. If a pull request from a `sync/upstream-*` branch is open, reply with its link and stop.
2. Work on a new branch `sync/upstream-<yyyy-mm-dd>` from `origin/main`. In the primary checkout
   (`git rev-parse --git-dir` is `.git`) keep its branch as it is and use
   `git worktree add ../upstream-sync -b <branch> origin/main`. Wait for worktree setup as
   `AGENTS.md` describes.
3. `node scripts/upstream-sync.ts > .data/upstream-report.md`, then read the whole report. If it
   says "Already up to date", reply with that line and stop.

## 2. Fix the map

The report asks for a change to `scripts/upstream-map.ts` in three places: its "Fix" section, a
Dropped file we only moved, and a Dropped folder we moved or rewrote. A `MOVED` entry changes the
merge itself, so apply these before resolving anything: `git merge --abort`, edit the map, commit it
as `chore(upstream): <what moved>`, and run step 1.3 again.

Done when the report has no "Fix" section and every Dropped entry is something we cut or rewrote.

## 3. Resolve the conflicts

`git status` lists them. Each conflict shows three versions: ours, the merge base, and Modrinth's.
The difference between the last two is what Modrinth changed.

- Modrinth's structure wins. Take their layout, names and refactors, then re-apply our behavior on
  top.
- Ads, posthog and analytics, and the MOTD profanity filter stay removed. If Modrinth adds to or
  moves them, drop that part.
- When Modrinth's code imports something we deleted, cut the import the way earlier syncs did.
  `git log -i --merges --grep='merge modrinth' origin/main` lists them.
- A file Modrinth deleted and we changed is usually code Modrinth moved. Take the deletion and
  re-apply our change where the code went.

Done when `git diff --name-only --diff-filter=U` prints nothing.

## 4. Sort the new files

Go through the report's New files. Delete the ones that belong to something we cut (Modrinth
Hosting's business code, ads, analytics, Modrinth's own agent docs). A deleted file stays deleted in
every later sync. Fold a new migration into our schema, then delete it.

## 5. Commit the merge

`node scripts/upstream-sync.ts --continue` regenerates the lockfiles and makes the merge commit.
Keep its two parents: never rebase, squash or recreate it.

## 6. Check

Run focused checks on what changed: `node scripts/dev-runner.ts test <crate>...` for changed Rust
crates, and `vp run --filter <workspace> typecheck` plus `vp lint <files>` for changed workspaces. A
crate this machine cannot build (missing Java or WebKit) is left to CI; say so in the PR. Fix a
failure the merge caused with `git commit --amend --no-edit`, so the merge commit itself passes.

## 7. Port

To port lists the Modrinth changes that touch code we rewrote. Merging gave us none of them, so
each one is owed to our code until a trailer settles it. Read every change
(`git show <sha> -- <path>`) and give it one outcome:

- **Port it**, following the porting rules, in a commit titled like
  `fix(backend): port session expiry from upstream` that ends with `Upstream: <sha>`
  (`git commit --trailer "Upstream: <sha>"`). A port changes behavior, so it ships with the test
  `AGENTS.md` asks for. Every fix is ported in this PR.
- **Skip it** when nothing of ours needs it: `Upstream-skip: <sha> <why>`.
- **Put it off** only when it is a feature too large for this PR: `Upstream-later: <sha> <what it
  takes>`. The next sync lists it again.

A skip or a later goes on any commit of the branch, such as
`git commit --amend --no-edit --trailer "Upstream-skip: <sha> <why>"`. Port what the report lists
under "Still owed from earlier syncs" too, where this PR can carry it.

Done when `node scripts/upstream-sync.ts --ports` passes.

## 8. Keep big new features dormant

A big new feature adds a page, flow or service Amberite has not decided to ship. Keep all of its
code and hide only its entry points (route, nav item, button) in one small commit titled
`chore(upstream): keep <feature> dormant`, with a comment at each change starting
`// Dormant upstream feature:`. Small features and fixes ship as they are.

## 9. Push and open the PR

1. Push the branch.
2. Open a real PR (not a draft) titled `chore: merge Modrinth upstream through <Month day>`. For
   what Modrinth shipped, use the report's Commits, `git diff <base> <upstream> --
   packages/blog/changelog.ts` (both shas head the report), and `gh pr view <number> -R
   modrinth/code` for the changes worth describing. Sections, in order:
   1. **New features**: each feature, whether it shipped, was kept dormant, or was skipped, and what
      enabling or porting it would take.
   2. **Ported**: each port with its commit, then every change `--ports` lists as later, with what
      it takes.
   3. **Fixes you get for free**: one line each.
   4. **Cut**: one line with the number of `cut` commits.
   5. **Conflicts**: one line per file or group of files.
3. End the body with "Merge with a merge commit. Never squash or rebase: the next sync needs both
   parents." and a line naming the model and harness.
4. Reply with the PR link.
