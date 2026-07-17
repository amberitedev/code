---
name: prototype
description: create a ui prototype from a notion page.
---

# Prototype UI

Create a standalone prototype that lets the user judge the ui and ux faster than the fully finished porduct, not a wireframe. The UI should look exactly as if it was the fully implemented one, but with cheap workarounds for not having a backend. 

## 1. Require a dedicated prototype branch

Before doing anything, require a dedicated slice worktree/branch; if the task is on `main` or outside one, stop and tell the user, otherwise work on `<current-branch>/prototype` (without appending `/prototype` twice). Never work on `main` or a branch that does not end in `/prototype`.

## 2. Understand before editing

1. Read the entire linked Notion slice and the central workflow page when provided.
2. Inspect the current implementation, adjacent pages, navigation, and `packages/ui`. Identify the real shell, components, tokens, density, and interaction patterns the feature must use.
3. Build a decision inventory covering the observable outcome, entry point, actors and permissions, primary journey, content and hierarchy, important interactions, success destinations, major states, and in-scope and out-of-scope behavior.
4. Ask every unresolved question that could materially change the product, UI, or UX. Ask in manageable batches and recommend an option when useful. Continue until the feature is understood well enough that you are no longer inventing product decisions.
5. Summarize the agreed experience before building. Do not edit prototype code while material product decisions remain unresolved.

## 3. Build the interactive prototype

1. Make the UI look as close to perfect as possible and roughly 85% final: use the real Amberite shell and `@modrinth/ui`, and do not cut corners in visible UI or UX.
2. Show the richest important state by default, with realistic content that fills the page and exposes its meaningful UI instead of choosing an empty, minimal, or generic default.
3. Build additional states only when they materially change what the user needs to judge, such as viewing a profile as a stranger versus a friend. Add a small dev-only control in the bottom-right using real UI components so the user can switch between those states.
4. Make the journey interactive with plainly named page-local state and direct transitions so future agents can easily understand the intended UX logic. Skip persistence, network calls, backend work, migrations, server authorization, production architecture, and broad tests; add ghost/loading UI or optimizations only when they materially affect the experience being reviewed.

## 4. Verify and iterate

1. Start the app from the worktree when needed and use computer use for one quick pass over the primary flow and important variants. Fix obvious visual problems, dead controls, and awkward interactions; do not perform exhaustive QA or inspect every possible state.
2. Present the prototype with its state switcher, disclose simulated or omitted behavior, and iterate on the user's requested UI and UX changes until explicit approval.

## Boundaries

- Keep prototype code simple, readable, and local so another agent can understand every UX state and transition.
- The UI and UX may not be disposable in quality. Do not hand off a static mock, generic layout, dead controls, placeholder component system, or merely attractive screen.
- Do not disguise fake behavior as production behavior.
- Do not open a pull request, merge the prototype branch, invoke PR babysitting, or begin production implementation.
- Do not treat visual polish as approval. The user owns the product, UI, and UX decision.
- Do not write any backend code or any code outside of the ui.

 
