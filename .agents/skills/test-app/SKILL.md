---
name: test-app
description: Check a change in the real Amberite App from a browser tab, with scenario test accounts, and hand a window to the developer. Use for any App change a person can see.
---

# Test the App

## Start it

`node scripts/dev-runner.ts up 1` (or `1 2` for two accounts) starts the stack in the background
and prints each App's `tab:` URL once it loads. There is no `vp run up`: on Windows, vp kills the
background stack when it returns. `vp run status` and `vp run down` are safe through vp, because
they start nothing that outlives them. Reuse a healthy stack from earlier in the task:
`vp run status` prints the URLs again. Stop only what you started, with `vp run down`.

When Rust has to build, the first start takes several minutes. `up` gives up waiting after 9
minutes; run it again to keep waiting.

## Drive it

- Open each App's `tab:` URL in the preview. Each tab is its own origin and talks to its own App,
  so two tabs are two accounts side by side. Do not open the bare frontend URL: it reaches no App,
  and the sign-in website hangs the preview tools.
- The page removes the `#local=` fragment from the address bar after it loads and keeps the App's
  address for the tab's session, so a reload still works. A new tab needs the full `tab:` URL.
- The preview tab is hidden, so `requestAnimationFrame` never runs and page transitions stall.
  After each load, replace it in the page:
  `window.requestAnimationFrame = (f) => setTimeout(() => f(performance.now()), 16); window.cancelAnimationFrame = clearTimeout`.
- Each App signs itself in as `scenario_<number>`, so screens behind an account work at once.
- Use data that reaches the changed screen. Scenario 1's account has the richest data.
- A full-screen "Something broke" (`[data-crash-screen]`) means a page threw while rendering. Its
  Details and the console's `[crash]` line hold the route and stack.
- Only the native window shows window chrome, system dialogs, and drag and drop from the desktop.
  A dialog started from a tab opens on the desktop. Those need a person, or computer use with the
  developer's go-ahead.

## Hand it over

When the developer wants to look themselves: `vp run down`, then
`node scripts/dev-runner.ts up <scenarios> --show-windows`, and say which window and screen to look
at. Once the developer has a window, do not drive that App.
