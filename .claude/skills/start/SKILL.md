---
name: start
description: Start a work session. Pick the next Linear issue, settle its open decisions with the user, build it, and open a PR with screenshots. Use when the user runs /start.
disable-model-invocation: true
argument-hint: "An issue like AMB-12, or nothing to pick the next one"
---

# Start

One issue per session. Ilai owns every product and UI decision; you own the facts, the code, and
the screenshots. Do not ask Ilai what to work on: pick, say what you picked, and go. Linear labels,
statuses, and operations are in `docs/agents/issue-tracker.md`; read it first, with `CONTEXT.md`.

## 1. Pick

1. If Ilai named an issue, take it.
2. Otherwise, in team `AMB`:
   1. An issue In Review whose PR or Linear issue has feedback newer than the last push: continue it
      at step 5.
   2. Else the first Todo issue whose blockers are all Done, with a `ready-for-agent` or
      `needs-info` label, highest priority first, then smallest estimate.
   3. Never pick Backlog, `needs-triage`, or `ready-for-human` issues.
3. Tell Ilai in one or two lines which issue and why. Also list any open `ready-for-human` issues
   as "only you can do these". Then carry on; Ilai can interrupt to swap.
4. Assign the issue to `me` and move it to In Progress.

## 2. Settle the open decisions

Skip this step for `ready-for-agent` issues.

1. Read the code and screens the issue touches. For anything visible, start the App
   (`node scripts/dev-runner.ts up 1`, or `1 2` when two accounts matter) and screenshot the
   current state through the browser bridge; these are the "before" images.
2. Call the Skill tool for `grilling` and `domain-modeling`. The issue's Open decisions are the
   first round. Number each question and give your recommendation. Look up facts yourself; only
   decisions go to Ilai.
3. When a question is about layout or flow, show it: an HTML page (`html-communication`, published
   with `html_render`) with labeled variants `A`, `B`, `C` drawn to look like the real App. Follow
   `docs/internals/ui.md`.
4. Repeat until Ilai says it is settled. Write no product code before that.
5. Post an agent brief comment (`.claude/skills/triage/AGENT-BRIEF.md`), fold the answers into the
   issue's Done when, remove its Open decisions, and switch the label to `ready-for-agent`.

If the session ends before everything is settled, post triage notes (see the `triage` skill), keep
`needs-info`, and move the issue back to Todo.

## 3. Build

1. Work in a new worktree from the latest `origin/main`, on the issue's `gitBranchName`. Never on
   `main`.
2. Build the whole issue: UI, `packages/api-client`, and the Core or backend changes it needs. Hit
   every surface in `AGENTS.md`: both ends, reverse states, consumers of shared UI.
3. New problems you find go to Linear as new issues with `needs-triage`, not into this branch.
4. Light checks only: typecheck and lint the workspaces you touched, and tests you touched.

## 4. Open the PR

1. For anything visible, use the changed screen through the browser bridge and screenshot every
   state in Done when. Upload them with `file-upload`.
2. Open a real PR with `file-pr`: the problem, how it behaves now, before and after screenshots, and
   how to try it. The body includes `Fixes AMB-<n>`.
3. Move the issue to In Review if Linear has not already.
4. Leave the App running and give Ilai its bridge URL.

## 5. Iterate

1. Ilai gives feedback in chat, on the PR, or on the Linear issue. Fix it, push, and replace the
   screenshots that changed.
2. Repeat until Ilai merges. Never merge yourself.
3. After the merge, check the issue moved to Done, stop the dev stack with
   `node scripts/dev-runner.ts down`, and end the session. The next session is another `/start`.
