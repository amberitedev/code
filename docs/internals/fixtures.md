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
