---
name: implement
description: implenent a prototype into a complete production vertical slice, compare it with the prototype, create a pull request, and babysit the PR. Use only when explicitly requested after prototype approval.
---



## Before Implementation

1. Inspect the approved prototype. Derive the original branch by removing the final `/prototype` segment from the prototype branch name.
2. Ask the user concise questions about unresolved behavior, acceptance criteria, data, permissions, and failure states. Wait for answers before making production changes.
3. Capture the approved prototype states needed for later comparison.
4. While on the prototype branch, commit all prototype content, including untracked files,  and push the prototype branch. Verify the worktree is clean and the remote contains the commit before switching to the derived original branch. If the commit or push fails, stop without switching so no prototype work can be lost. Do not create another branch or copy the prototype wholesale into production.

## Implement the Slice

1. Define a complete vertical slice that delivers the approved flow, including every required UI state, API contract, persistence change, authorization rule, Core or Convex behavior, and failure path.
2. Implement that slice on the original branch. Use the prototype as the UX and UI source of truth and existing project patterns for production architecture.
3. Keep the scope tight. do not branch in to unafected and unrealted features. 

## Test with Development State

1. if any changes to convex have been made use convex local deplyment. while testing use the corresponding porifle for the aria of ui you are working on, 

## Review and Handoff

1. use computer use to exercise the whole flow in the running desktop app from every relevant account angle. Compare the layout, states, interactions, permissions, and failure behavior with the approved prototype.
2. Fix every in-scope bug or prototype mismatch found by computer use, run the scoped checks again, and repeat the whole flow. repate untill all bugs are fixed and flow works corrcetly
3. Create a pull request after your done testing. than babysit the pr (useing `babysit-pr`) 
4. Use the root development-state commands to ensure the app for the original branch is running and focused for the user.
5. Report the PR, checks, comparison results, babysitting results, remaining risks, and exact steps the user should follow to test the feature.
