#!/usr/bin/env node

// `vp run scenarios [e2e run arguments]`: drives the real App through the scenarios in
// packages/fixtures/scenarios, on the dev stack of the checkout it is run from. Starts the stack
// when none runs and leaves it up, holding the last scenario's data.
// See docs/internals/testing.md#scenarios.

import * as NodeChildProcess from 'node:child_process'
import * as NodePath from 'node:path'
import * as NodeURL from 'node:url'

import { STACK_ENV, type StackEnv } from '../packages/fixtures/scenarios/stack-env.ts'
import { connectStack, parseInput, resolveWorktreePaths } from './dev-runner.ts'

// The scenarios and e2e are this file's checkout's. The stack is the working directory's, which
// `scenarios:prove` points at a checkout of the base commit.
const fixtures = NodeURL.fileURLToPath(new URL('../packages/fixtures', import.meta.url))
const paths = resolveWorktreePaths()
// Apps sign themselves in and are driven from browser tabs, as for a fixture.
const stack = connectStack(paths, { ...parseInput([]), accountWeb: false, hideWindows: true })
const runtime = await stack.answering()

const env: StackEnv = {
	appUrl: runtime.urls.app,
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
