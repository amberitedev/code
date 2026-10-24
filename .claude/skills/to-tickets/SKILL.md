---
name: to-tickets
description: Break a plan, spec, or the current conversation into tracer-bullet Linear issues, each with its type, triage label, estimate, and native blocking relations.
disable-model-invocation: true
---

# To Tickets

Adapted from [mattpocock/skills](https://github.com/mattpocock/skills) (MIT).

Break a plan, spec, or conversation into **tickets**: tracer-bullet vertical slices, each declaring
the tickets that **block** it. Linear, labels, and the issue template are in
`docs/agents/issue-tracker.md`; read it first.

## Process

### 1. Gather context

Work from what is already in the conversation. If Ilai passes a reference (a doc path, an issue
like `AMB-12`), fetch it and read its full body and comments.

### 2. Explore the code (optional)

If you have not explored the code yet, do. Titles and descriptions use `CONTEXT.md` terms and
respect ADRs in `docs/adr/`. Check existing Linear issues so you do not file duplicates.

Look for prefactoring that makes the work easier: "make the change easy, then make the easy
change."

### 3. Draft vertical slices

- Each slice cuts a narrow but complete path through every layer it needs (Core, backend,
  `packages/api-client`, UI). Vertical, not one layer at a time.
- A finished slice can be demoed or verified on its own.
- Each slice fits one fresh agent session (estimate 3 or less; split anything bigger).
- Prefactoring comes first.

Give each ticket its **blocking edges**: the tickets that must be Done before it can start.

**Wide refactors are the exception.** One mechanical change whose blast radius crosses the whole
repo (a rename of a shared type) cannot land as a green vertical slice. Sequence it as
expand-contract: add the new form beside the old, migrate callers in batches (each blocked by the
expand), then delete the old form in a ticket blocked by every batch.

For each ticket decide its Triage label: `ready-for-agent` when nothing is left to decide,
`needs-info` when Ilai still has to decide something (list those under Open decisions, each with
your recommendation), `ready-for-human` when only Ilai can do it.

### 4. Quiz Ilai

Show the breakdown as a numbered list. For each ticket:

- **Title**
- **Type / Triage / estimate**
- **Blocked by**
- **What it delivers**: the end-to-end behaviour it makes work

Ask whether the granularity is right, whether the blocking edges are real, and whether anything
should merge or split. Iterate until Ilai approves. If Ilai already asked you to just file them,
file them and show the list afterwards.

### 5. Publish to Linear

Create one issue per ticket in dependency order (blockers first), so `blockedBy` can name real
identifiers. Use the issue template from `docs/agents/issue-tracker.md`. Put each issue in the
Linear project it belongs to, if any. Something outside Linear that blocks a ticket (an upstream
PR) goes in the description and as a link, with the ticket in Backlog.

The **frontier** is every ticket whose blockers are all Done.

Do not close or modify a parent issue.

Avoid file paths and code snippets in tickets: they go stale. Exception: a type shape or state
machine that encodes a decision more precisely than prose can.
