# Fixtures

> For maintainers and agents.

A fixture is a TypeScript file that seeds the dev environment to what you describe, for easier
testing.

`vp run fixture <file>` wipes the stack and runs the file from the top against the current code.
Ready-made ones are in `packages/fixtures/fixtures` and run by name, like `vp run fixture default`.

## How it works

- `stack({ apps, users, cores })` is the setup: who has an App, which accounts exist without one,
  and the Cores with their owners. It returns a handle for each.
- `call(who, endpoint, params, body)` sends one endpoint to an App, to the backend as an account,
  or to a Core. Endpoints are in `packages/contracts/ts`. The program that owns a state makes it.
- `patch(store, table, where, values)` changes rows a program already wrote, for a state no
  endpoint reaches, such as an expired link. It shows how the App handles a state, never that a
  program gets there.
- `sql` reads rows, and `until` waits for background work.

## Helpers

A helper is a function that does a bunch of fixture steps for you, so a fixture says it in one
line: `instance(alice, {...})` instead of ten calls. They are in `packages/fixtures/kit`; look
there before writing steps by hand.

Add one when setting something up takes a lot of steps, like an instance with mods, or goes
through several programs, like pairing a Core. One or two calls do not need one. If what you built
is hard to set up for testing, write its helper in the same change.

A helper does things the way the App does them and waits until they are finished.

## Scenarios

A scenario is a fixture that goes on: it seeds the stack, then drives the real App in a browser
and checks what the user sees. Scenarios are `*.e2e.ts` files in `packages/fixtures/scenarios`,
run by [e2e](https://e2e.com/docs). Use one as real-app proof for a bug fix a user could reproduce:

```sh
vp run scenarios scenarios/<name>.e2e.ts                            # run one while you write it
vp run scenarios:prove packages/fixtures/scenarios/<name>.e2e.ts    # prove a fix
```

Nothing runs them all on a schedule, and CI does not run them. Each one is written with a fix,
proves it once, and stays as a way to see that behavior again.

- One test per file, with `test` and the fixture tool from `./amberite.ts`.
- `amberite.seed(...)` takes what a fixture file holds and returns what it builds, so the test
  keeps its Apps, Cores and ids. `amberite.open(app, path)` opens that App's tab at a route, and
  `amberite.stop(core)` turns a program off.
- Open the page under test directly with `amberite.open`, unless the scenario is about getting
  there.
- Put `expect` assertions only on the behavior the bug breaks, and take `app.screenshot('label')`
  where the bug shows, so a proof has the capture from both sides. Never call `agent.act` or
  `agent.assert`: no model is configured.
- Screenshots are the proof. Every run also records a video, kept with them and a `trace.md` of
  every step under `packages/fixtures/.e2e/results`: use it only when a still cannot show the
  behavior, such as motion or order, or when the developer asks for it.
- A run replaces the stack's data, as applying a fixture does, and leaves the stack up on the last
  scenario's. Run one at a time per checkout.
- `scenarios:prove` runs the scenario on this checkout as it is, uncommitted changes included,
  then on the merge base with `origin/main` (or `--base <ref>`) in a temporary worktree beside this
  one. It exits `0` only when the scenario fails on base and passes here (`1` = did not prove,
  `2` = the App did not run, `3` = the scenario stopped before its assertion). The scenario and the
  fixture tool are always this checkout's, so base only supplies the programs. It writes
  `packages/fixtures/.e2e/prove/proof.md`, with the captures of both sides.
  `--upload-command "<command>"` runs that command with the capture files as arguments and puts
  the URLs it prints, one a line, in the proof in place of the local paths.
- Scenarios drive an App's browser tab. They cannot cover the App's own window, a running game,
  or timing.
