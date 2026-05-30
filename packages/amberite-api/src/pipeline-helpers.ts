import type { CommunicationMethod, CommunicationPolicy } from './pipeline-types'
import type { MessageDefinition } from './transport'

export function toMessageMode(method: CommunicationMethod): MessageDefinition['mode'] {
	if (method === 'core-relay') return 'core-relay'
	if (method === 'convex-relay') return 'convex-relay'
	if (method === 'memory-queue' || method === 'persistent-queue') return 'direct-queued'
	return 'direct-fire-and-forget'
}

export function withCommunicationTimeout<T>(
	fn: (signal: AbortSignal, policy: CommunicationPolicy) => Promise<T>,
	policy: CommunicationPolicy,
): Promise<T> {
	const controller = new AbortController()
	let timer: ReturnType<typeof setTimeout> | null = null
	if (policy.timeoutMs > 0) {
		timer = setTimeout(() => controller.abort(), policy.timeoutMs)
	}
	return fn(controller.signal, policy).finally(() => {
		if (timer) clearTimeout(timer)
	})
}

export function sleep(ms: number): Promise<void> {
	return new Promise((resolve) => setTimeout(resolve, ms))
}
