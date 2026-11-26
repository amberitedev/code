// How `scripts/scenarios.ts`, which starts the dev stack, tells the tests where it is.

import type { Runner } from '../src/context.ts'

export const STACK_ENV = 'AMBERITE_SCENARIO_STACK'

export interface StackEnv extends Pick<Runner, 'data' | 'primary' | 'worktree'> {
	/** The App frontend's dev server. */
	readonly appUrl: string
}

export function stackEnv(): StackEnv {
	const value = process.env[STACK_ENV]
	if (!value) throw new Error('Scenarios run through the dev stack: vp run scenarios')
	return JSON.parse(value) as StackEnv
}
