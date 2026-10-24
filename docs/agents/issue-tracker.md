# Issue tracker: Linear

Work for this repo lives in Linear, team `AMB` (Amberitedev), through the Linear MCP server
(`mcp__linear__*` tools). GitHub Issues are off. The `start`, `triage`, and `to-tickets` skills read
this file.

## Labels

Every issue carries exactly one **Type** label and one **Triage** label. Both are single-select
groups, so Linear keeps them to one each.

| Type      | Meaning                                  |
| --------- | ---------------------------------------- |
| `feature` | Something new a user can do              |
| `fix`     | Something that is broken or misleading   |
| `cleanup` | Remove or simplify something that exists |

| Triage            | Meaning                                                                |
| ----------------- | ---------------------------------------------------------------------- |
| `needs-triage`    | Not evaluated yet                                                      |
| `needs-info`      | Open: decisions from Ilai are needed before an agent can build it      |
| `ready-for-agent` | Closed: fully specified, an agent can build it without asking anything |
| `ready-for-human` | Only Ilai can do it (accounts, hardware, the real desktop)             |

Area labels, any number: `servers`, `core`, `pairing`, `sharing`.

Rejected work (`wontfix` in the upstream skills) is the **Canceled** status with a comment saying
why, not a label.

## Statuses

- **Backlog**: waiting on something outside Linear (an upstream PR, Ilai's "not now"). Never picked.
- **Todo**: can be picked once every blocker is Done.
- **In Progress**: claimed by a session. Assign it to Ilai (`me`) when claiming.
- **In Review**: a PR is open.
- **Done**: the PR merged. Linear's GitHub integration moves it there when the PR body says
  `Fixes AMB-<n>`.
- **Canceled**: will not be done.

## Writing an issue

Use the domain terms from `CONTEXT.md`. Describe behaviour, not files and line numbers; name types
and functions only where they help find the code.

```markdown
## Problem

What the user runs into today, from their point of view.

## Done when

- [ ] Testable criterion, from the user's point of view
- [ ] Include the way back out and the way to see the state

## Open decisions

1. A decision only Ilai can make. (Recommended: your answer.)

## Out of scope

- What this issue does not touch
```

Leave out **Open decisions** on `ready-for-agent` issues. Set **estimate** (1, 2, 3, 5, 8 points;
one session should fit a 3) and, when it matters, **priority**.

## Operations

- **Create or update**: `save_issue` with `team: "AMB"`, `labels`, `estimate`, `state`, and
  `project` when the issue belongs to one.
- **Blocking**: native relations, `blockedBy` on `save_issue`. An issue is unblocked when every
  blocker is Done. Something outside Linear that blocks (an upstream PR) goes in the description
  and as a link, with the issue in Backlog.
- **Read**: `get_issue` with `includeRelations: true`; `list_comments` for the discussion.
- **List**: `list_issues` with `team: "AMB"` and `state`, `label`, or `project` filters.
- **Comment**: `save_comment`.
- **Branch**: the issue's `gitBranchName` (for example `ilaigibb/amb-12-...`). Linear links a PR
  from that branch to the issue.
- **Close**: through the PR. The PR body says `Fixes AMB-<n>`; merging moves the issue to Done.

## When a skill says "publish to the issue tracker"

Create a Linear issue in team `AMB` as above.

## When a skill says "fetch the relevant ticket"

`get_issue` with relations, then `list_comments`.
