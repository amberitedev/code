#!/usr/bin/env node

// `vp run scenarios [e2e run arguments]`: drives the real App through the scenarios in
// packages/fixtures/scenarios, on the dev stack of the checkout it is run from. Uses the running
// stack, which keeps the last scenario's data; with none running, starts one and stops it after.
// See docs/internals/testing.md#scenarios.

import * as NodeChildProcess from 'node:child_process'
import * as NodePath from 'node:path'
import * as NodeURL from 'node:url'

import { STACK_ENV, type StackEnv } from '../packages/fixtures/scenarios/stack-env.ts'
import { connectStack, fixtureTurn, holdStack, resolveWorktreePaths } from './dev-runner.ts'

// The scenarios and e2e are this file's checkout's. The stack is the working directory's, which
// `scenarios:prove` points at a checkout of the base commit.
const fixtures = NodeURL.fileURLToPath(new URL('../packages/fixtures', import.meta.url))
const paths = resolveWorktreePaths()
const held = await holdStack(paths)
let endTurn: (() => void) | undefined
try {
	const stack = connectStack(paths)
	// Seeding replaces the stack's data, as a fixture does, so the run takes the fixture's turn.
	endTurn = await fixtureTurn(paths.data)
	// A scenario opens the App as soon as it has seeded, which a new checkout's frontend is not ready for.
	await stack.pageLoads()

	const env: StackEnv = {
		appUrl: stack.live.urls.app,
		data: paths.data,
		primary: paths.primary,
		worktree: paths.worktree,
	}
	const run = NodeChildProcess.spawnSync(
		process.execPath,
		[
			NodePath.join(fixtures, 'node_modules', 'e2e', 'dist', 'cli', 'bin.js'),
			'run',
			...process.argv.slice(2),
		],
		{
			cwd: fixtures,
			env: { ...process.env, E2E_TELEMETRY_DISABLED: '1', [STACK_ENV]: JSON.stringify(env) },
			stdio: 'inherit',
			windowsHide: true,
		},
	)
	process.exitCode = run.status ?? 1
} finally {
	endTurn?.()
	await held.release()
}
