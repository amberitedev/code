// Scenarios: tests that drive the real App on this checkout's dev stack. `vp run scenarios` starts
// the stack and runs this; see docs/internals/testing.md#scenarios.
import { web } from '@e2e-dev/web'
import type { E2EConfig } from 'e2e'

import { stackEnv } from './scenarios/stack-env.ts'

export default {
	tests: 'scenarios/**/*.e2e.ts',
	targets: [
		{
			name: 'web',
			engine: web({ viewport: { width: 1280, height: 800 } }),
			// A scenario opens each App at its own tab URL; this is only where relative paths would go.
			app: { url: stackEnv().appUrl },
		},
	],
	// Every scenario replaces the one stack's data.
	workers: 1,
	// A first server install downloads Minecraft and Java.
	timeout: 20 * 60_000,
	assertionTimeout: 15_000,
	video: 'on',
	trace: 'on',
	reporters: ['list', 'markdown'],
} satisfies E2EConfig
