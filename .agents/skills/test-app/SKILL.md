---
name: test-app
description: Test the Amberite App through T3's built-in Browser panel against this checkout's dev stack. Use for App verification, browser recovery, and test fixtures.
---

# Test the App

Use T3's built-in Browser panel for verification. Follow the recovery steps below before declaring
it unavailable. Do not install or switch to another automation system.

## Start the app

Reuse this task's stack: `vp run status` prints each App's `tab:` URL. Otherwise start one as a
background task from the repository root with `vp run dev <fixture> --hide-windows`. It runs on
this checkout's ignored `.data` and prints the URLs when it is ready. The bare frontend URL reaches
no App.

Test with meaningful state. Describe the state that lets you test the scenario you want as a
[fixture](../../../docs/internals/fixtures.md) and apply it with `vp run fixture <name>`, rather
than clicking state together.

## Use the Browser panel

Call `preview_status`. If no tab can be automated, call `preview_open`, including when status
reports `available: false` or `tabId: null`. Status can report unavailable simply because no tab
exists yet; opening a tab needs no user action. Stop verification and explain the blocker only if
the tools are absent or `preview_open` reports unavailable or fails.

Open each App's `tab:` URL in its own tab, then use `preview_snapshot` and T3's interaction tools.
Each tab is one account, signed in. A tab keeps its URL across fixtures; reload it after one.

Snapshots and clicks need the T3 Code window on screen. If they time out, ask for the window and
continue through `preview_evaluate`.

## Verify and stop

Exercise the affected flow in every account it reaches and capture the state that proves it works.

Stop the stack when you are done by stopping its task, or with `vp run down`: this machine runs
several agents. Leave nothing running for the developer; tell them the `vp run dev <fixture>` that
shows the change.
