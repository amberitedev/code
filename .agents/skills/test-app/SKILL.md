---
name: test-app
description: Check a change in the real Amberite App from a browser tab, on a fixture's test accounts, and hand a window to the developer. Use for any App change a person can see.
---

# Test the App

## Start it

`node scripts/dev-runner.ts fixture <name>` starts the stack in the background, builds the
fixture's state, and prints each App's `tab:` URL once it loads. Without a name it applies
`default`: Apps for `alice` and `bob`, who are friends, a Core `alice` owns, and an instance she
shares with `bob`. When the change needs another state, write that state as a fixture (see
Fixtures in AGENTS.md) and apply the file. There is no `vp run fixture`: on Windows, vp kills the
background stack when it returns. `vp run status` and `vp run down` are safe through vp, because
they start nothing that outlives them. Reuse a healthy stack from earlier in the task:
`vp run status` prints the URLs again. Stop only what you started, with `vp run down`.

When Rust has to build, the first start takes several minutes. The command gives up waiting after
9 minutes; run it again to keep waiting.

## Drive it

- Open each App's `tab:` URL in the preview. Each tab is its own origin and talks to its own App,
  so two tabs are two accounts side by side. Do not open the bare frontend URL: it reaches no App,
  and the sign-in website hangs the preview tools.
- The page removes the `#local=` fragment from the address bar after it loads and keeps the App's
  address for the tab's session, so a reload still works. A new tab needs the full `tab:` URL.
- Applying a fixture restarts every App. A tab that was open is on an old session: open its
  `tab:` URL again.
- The preview tab is hidden, so `requestAnimationFrame` never runs and page transitions stall.
  After each load, replace it in the page:
  `window.requestAnimationFrame = (f) => setTimeout(() => f(performance.now()), 16); window.cancelAnimationFrame = clearTimeout`.
- Each App signs itself in as the account the fixture names, so screens behind an account work at
  once.
- Use a fixture whose state reaches the changed screen.
- A full-screen "Something broke" (`[data-crash-screen]`) means a page threw while rendering. Its
  Details and the console's `[crash]` line hold the route and stack.
- Only the native window shows window chrome, system dialogs, and drag and drop from the desktop.
  A dialog started from a tab opens on the desktop. Those need a person, or computer use with the
  developer's go-ahead.

## Hand it over

When the developer wants to look themselves: `vp run down`, then
`node scripts/dev-runner.ts up --show-windows`, which starts the same Apps on the same data with
their windows, and say which window and screen to look at. Once the developer has a window, do not
drive that App.
