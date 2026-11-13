import { ContractClient, type LocalEvent, ops } from '@modrinth/api-client'

import { localTransport } from '@/platform/local'

export type OpProgress = Extract<LocalEvent, { type: 'progress' }>['payload']
export type OpEnd = Extract<LocalEvent, { type: 'op_ended' }>['payload']

const local = new ContractClient(localTransport)
const running = new Map<
	string,
	{ progress?: (progress: OpProgress) => void; end: (end: OpEnd) => void }
>()

/** Hands the App's op events to the `runOp` waiting on them. */
export function dispatchOpEvent(event: LocalEvent) {
	if (event.type === 'progress') running.get(event.payload.op)?.progress?.(event.payload)
	else if (event.type === 'op_ended') running.get(event.payload.op)?.end(event.payload)
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
	const ended = new Promise<OpEnd>((end) => running.set(op, { progress, end }))
	try {
		await start()
		return await ended
	} finally {
		running.delete(op)
	}
}

/** The op ends with `cancelled`. */
export async function cancelOp(op: string) {
	await local.call(ops.cancelOp, { params: { id: op } })
}
