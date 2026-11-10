// One line per setup, build, and start, in the primary checkout's `.data/timings.jsonl`, shared by
// every checkout. It is how anyone checks whether a change to the dev setup made it faster.

import * as NodeFS from 'node:fs'
import * as NodePath from 'node:path'

export type Timing = {
	readonly kind: 'setup' | 'build' | 'up'
	readonly worktree: string
	readonly result: 'ok' | 'failed' | 'cancelled' | 'waiting'
	/** Milliseconds per step, plus `total`. */
	readonly ms: Readonly<Record<string, number>>
	readonly [detail: string]: unknown
}

/** Appends `timing`. A log that cannot be written never fails the step it describes. */
export function recordTiming(primary: string, timing: Timing): void {
	try {
		const file = NodePath.join(primary, '.data', 'timings.jsonl')
		NodeFS.mkdirSync(NodePath.dirname(file), { recursive: true })
		// One spelling per worktree, so lines group by it.
		const line = {
			at: new Date().toISOString(),
			...timing,
			worktree: NodePath.resolve(timing.worktree),
		}
		NodeFS.appendFileSync(file, `${JSON.stringify(line)}\n`)
	} catch {
		// Read-only or missing primary checkout.
	}
}
