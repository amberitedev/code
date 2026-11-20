---
name: test-app
description: Test the Amberite App through T3's built-in Browser panel against this checkout's dev stack. Use for App verification, browser recovery, and test fixtures.
---

# Test the App

Use T3's built-in Browser panel for verification. Follow the recovery steps below before declaring
it unavailable. Do not install or switch to another automation system.

## Start the app

Reuse this task's healthy stack: `vp run status` prints each App's `tab:` URL. Otherwise run
`vp run fixture` from the repository root, which starts the stack in the background on this
checkout's ignored `.data` and prints them. Read the actual URLs from that output. The bare
frontend URL reaches no App.

Test with meaningful state. Describe the state that lets you test the scenario you want as a
[fixture](../../../docs/internals/fixtures.md) and apply it, rather than clicking state together.

## Use the Browser panel

Call `preview_status`. If no tab can be automated, call `preview_open`, including when status
reports `available: false` or `tabId: null`. Status can report unavailable simply because no tab
exists yet; opening a tab needs no user action. Stop verification and explain the blocker only if
the tools are absent or `preview_open` reports unavailable or fails.

Open each App's `tab:` URL in its own tab, then use `preview_snapshot` and T3's interaction tools.
Each tab is one account, signed in. After a fixture is applied, open the new URLs.

Snapshots and clicks need the T3 Code window on screen. If they time out, ask for the window and
continue through `preview_evaluate`.

## Verify and stop

Exercise the affected flow in every account it reaches and capture the state that proves it works.

Stop the stack with `vp run down` when you are done: this machine runs several agents. Then run
`vp run fixture --show-windows`, which starts a clean stack with App windows for the developer to
try the change, and leave those Apps to them.
