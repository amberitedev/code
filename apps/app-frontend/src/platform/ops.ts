import { ContractClient, type LocalEvent, ops } from '@modrinth/api-client'

import { localTransport } from '@/platform/local'

export type OpProgress = Extract<LocalEvent, { type: 'progress' }>['payload']
export type OpEnd = Extract<LocalEvent, { type: 'op_ended' }>['payload']

const local = new ContractClient(localTransport)
const running = new Map<
	string,
	{
		started: boolean
		progress?: (progress: OpProgress) => void
		end: (end: OpEnd) => void
	}
>()

/** Hands the App's op events to the `runOp` waiting on them. */
export function dispatchOpEvent(event: LocalEvent) {
	if (event.type === 'progress') running.get(event.payload.op)?.progress?.(event.payload)
	else if (event.type === 'op_ended') running.get(event.payload.op)?.end(event.payload)
	else if (event.type === 'resync') void endLostOps()
}

/** A resync may have dropped an op's end: a started op the App no longer runs ends as lost. */
async function endLostOps() {
	const started = [...running].filter(([, op]) => op.started)
	if (started.length === 0) return
	const listed = await local.call(ops.listOps).catch(() => null)
	if (!listed) return
	for (const [op, { end }] of started) {
		if (listed.includes(op)) continue
		end({
			op,
			error: { error: 'unknown', description: 'Lost track of the operation' },
			response: null,
		})
	}
}

/**
 * Runs a long App operation: `start` sends its route with `op`, and the promise resolves with how
 * the op ended. Rejects only when the start is refused.
 */
export async function runOp(
	op: string,
	start: () => Promise<void>,
	progress?: (progress: OpProgress) => void,
): Promise<OpEnd> {
	const ended = new Promise<OpEnd>((end) => running.set(op, { started: false, progress, end }))
	try {
		await start()
		const entry = running.get(op)
		if (entry) entry.started = true
		return await ended
	} finally {
		running.delete(op)
	}
}

/** The op ends with `cancelled`. */
export async function cancelOp(op: string) {
	await local.call(ops.cancelOp, { params: { id: op } })
}
