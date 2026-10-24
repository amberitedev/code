---
name: triage
description: Move Linear issues through the triage labels, categorise, verify, grill if needed, and write agent-ready briefs.
disable-model-invocation: true
---

# Triage

Adapted from [mattpocock/skills](https://github.com/mattpocock/skills) (MIT).

Move issues in Linear through a small state machine of triage labels. Labels, statuses, and Linear
operations are in `docs/agents/issue-tracker.md`; read it first.

## Labels

One **Type** label: `feature`, `fix`, or `cleanup`.

One **Triage** label:

- `needs-triage`: not evaluated yet
- `needs-info`: open, waiting on Ilai's decisions
- `ready-for-agent`: closed, fully specified, an agent can build it alone
- `ready-for-human`: only Ilai can do it

Rejecting an issue is the Canceled status with a comment, not a label.

An issue with no Triage label goes to `needs-triage` first. From there it moves to `needs-info`,
`ready-for-agent`, `ready-for-human`, or Canceled. `needs-info` moves to `ready-for-agent` once Ilai
has settled every open decision. Ilai can override at any time; flag transitions that look unusual
and ask before acting.

## Invocation

Ilai runs `/triage` and says what they want. Examples:

- "Show me anything that needs my attention"
- "Let's look at AMB-42"
- "Move AMB-42 to ready-for-agent"
- "What's ready for agents to pick up?"

## Show what needs attention

List, oldest first:

1. Issues with no Triage label.
2. `needs-triage` issues.
3. `needs-info` issues with comments from Ilai since the last triage notes.

Show counts and a one-line summary each. Let Ilai pick.

## Triage a specific issue

1. **Gather context.** Read the issue, its relations, and its comments. Parse prior triage notes so
   you do not re-ask settled questions. Explore the code using `CONTEXT.md` terms and respect ADRs
   in `docs/adr/`. Search for an existing implementation of the request by concept, not wording,
   and say where you looked. If it already exists, it is Canceled as already done (step 5).
2. **Recommend.** Give your Type and Triage recommendation with reasons, plus a short summary of the
   code involved. Wait for direction.
3. **Verify.** For a `fix`, reproduce it (through the browser bridge for App bugs; see `AGENTS.md`).
   Report: confirmed (with the code path), failed, or not enough detail.
4. **Grill (if needed).** If the issue needs fleshing out, call the Skill tool for `grilling` and
   `domain-modeling`, one round of questions at a time, updating `CONTEXT.md` and ADRs as decisions
   land. For layout or flow questions, show labeled variants with `html-communication`.
5. **Apply the outcome:**
   - `ready-for-agent`: post an agent brief comment ([AGENT-BRIEF.md](AGENT-BRIEF.md)) and remove
     the issue's Open decisions section, folding the answers into Done when.
   - `ready-for-human`: same structure as a brief, plus why an agent cannot do it.
   - `needs-info`: post triage notes (template below).
   - Canceled: comment why. If it already exists, point to where it lives.
   - `needs-triage`: apply the label, with a comment if there was partial progress.

## Quick override

If Ilai says "move AMB-42 to ready-for-agent", say what you are about to change, then do it. Skip
grilling. If no brief exists yet, ask whether to write one.

## Triage notes template

```markdown
## Triage notes

**Settled so far:**

- point 1

**Still open for Ilai:**

1. question 1 (Recommended: answer)
```

Put everything settled during grilling under "Settled so far" so the work is not lost. Questions
must be specific, each with a recommendation.
