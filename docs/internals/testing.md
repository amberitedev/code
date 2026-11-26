# Testing

Rules for writing tests, and for asking for them in review.

## What to test

- Test behavior that can break, such as two things happening at once, an operation that fails halfway, sharing and sync between the App, a Core and the backend, sign-in and permission checks, files and archives from other people, external formats (modpacks, Modrinth's API), and the logic a bug fix changed. Leave out tests for what types already guarantee, trivial logic, copy text, smoke runs, mock-call mirrors, callback wiring, and props rendered to static markup.
- Extend before you add. When an existing test or table covers the behavior, add a row or an assertion there. Write a new test only when no case fits.
- Test at one layer: the lowest one that fails on `main`. Do not repeat the case in a wrapper test, at each call site, or in App and Core copies of the same logic.
- Write a browser test only for behavior that needs a real browser: layout and scrolling, focus and pointer input, or a component wired to its composables. Logic a component calls gets a unit test beside it (`*.test.ts`). A browser test spends seconds loading the App's modules and styles before its first test runs; a unit test takes milliseconds.
- In review, ask for a missing test only when it would catch a real regression. Do not ask for one assertion per constant, label, or `match` arm. Flag a new test that breaks the rules above, and name the fix: fold it into the existing case or table, delete it, keep one layer, or move a browser test whose assertions would pass without rendering into a unit test.
- Core and backend behavior changes ship with focused tests for that behavior.
- Proof for a bug fix depends on the bug. Logic, data, state, and protocol bugs get a failing test first (the fewest that fail on `main` for that reason, or before the fix for a bug this branch introduced, in an existing case of the module's tests where one fits), then the fix. Purely visual bugs (spacing, colour, alignment, a layout shift) get before/after captures as their proof and no test: do not add UI tests for nitpicks, and do not ask for one in review when a capture already shows it. A Core- or backend-only fix with no visible surface ships the test plus a short runtime transcript.
- Proof goes in a comment on the pull request, not in its description. Its headline is what now works, in the words of someone using it. One sentence says what was tried on `main` and on the branch, and what `main` did wrong. Then before and after, side by side: two captures, or the few lines of output that differ. No run ids, test names or file paths. An agent posts it from the developer's account, so its last line is `<sub>🤖 <model> on behalf of <developer></sub>`.

## Waiting for work

- Never wait on sleeps or polling. Await the event that marks the milestone: the Core's event, the op's end, the channel's message. A test that needs a timeout to pass is wrong.
  - No per-test timeout argument or retry. The project timeout is the ceiling, and flaky tests are fixed, not retried.
  - No `Instant::now()`, `performance.now()` or `Date.now()` deltas. Assert on events and counts, not elapsed time.
  - `#[ignore]`, `it.skip` and friends need a tracking issue on the line above: `// quarantine: #1234 <why>`.

## App browser tests

- Browser tests are `*.browser.test.ts` files in `apps/app-frontend/src`, beside the component. They render it in Chromium with `render` from `vitest-browser-vue`, with the App's styles loaded, so sizes and overflow are real. `vp test run <file>` runs one. A machine needs Chromium once: `pnpm exec playwright install chromium`.
- A component that reads a provider gets it from a small wrapper component in the test, and one that links gets a memory router. See [`breadcrumbs.browser.test.ts`](../../apps/app-frontend/src/app/shell/title-bar/breadcrumbs.browser.test.ts).
- Assert through `expect.element` or `expect.poll`, never `requestAnimationFrame` or a timer.

## Scenarios

A scenario is a [fixture](fixtures.md) that goes on: it seeds the stack, then drives the real App in a browser and checks what the user sees. Scenarios are `*.e2e.ts` files in [`packages/fixtures/scenarios`](../../packages/fixtures/scenarios), run by [e2e](https://e2e.com/docs). Use one as real-app proof for a user-reproducible bug fix:

```sh
vp run scenarios scenarios/<name>.e2e.ts                            # run one while you write it
vp run scenarios:prove packages/fixtures/scenarios/<name>.e2e.ts    # prove a fix
```

Nothing runs them all on a schedule, and CI does not run them. Each one is written with a fix, proves it once, and stays as a way to see that behavior again.

- One test per file, with `test` and the fixture tool from `./amberite.ts`.
- `amberite.seed(...)` takes what a fixture file holds and returns what it builds, so the test keeps its Apps, Cores and ids. `amberite.open(app, path)` opens that App's tab at a route, and `amberite.stop(core)` turns a program off.
- Open the page under test directly with `amberite.open`, unless the scenario is about getting there.
- Put `expect` assertions only on the behavior the bug breaks, and take `app.screenshot('label')` where the bug shows, so `scenarios:prove` has the capture from both sides. Never call `agent.act` or `agent.assert` (no model is configured).
- Screenshots are the proof. Every run also records a video, kept with them and a `trace.md` of every step under `packages/fixtures/.e2e/results`: use it only when a still cannot show the behavior, such as motion or order, or when the developer asks for it.
- A run replaces the stack's data, as applying a fixture does, and leaves the stack up on the last scenario's. Run one scenario run at a time per checkout.
- `scenarios:prove` runs the scenario on this checkout as it is, uncommitted changes included, then on the merge base with `origin/main` (or `--base <ref>`) in a temporary worktree beside this one. It exits `0` only when the test fails on base and passes here (`1` = did not prove, `2` = the scenario could not run: the App did not run, or the command was used wrongly, `3` = the scenario cannot prove anything as written: it stopped before its assertion, or its file holds several tests). The scenario, the fixture tool and the dev runner are always this checkout's, so base only supplies the programs. It writes `packages/fixtures/.e2e/prove/proof.md` in the shape of that comment, with each capture beside the one base took under the same name. `--comment <pull request>` posts it with the captures, `--what "<sentence>"` is its one sentence, and `--by "<model>"` names you in its last line. `--upload-command "<command>"` runs that command with the capture files as arguments and puts the URLs it prints, one a line, in the proof in place of the local paths.
- Scenarios drive an App's browser tab. They cannot cover the App's own window, a running game, or timing.
