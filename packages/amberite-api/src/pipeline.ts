import type { PlatformAdapter } from './adapter'
import { AmberiteApiError, NetworkError } from './errors'
import { mergePolicy } from './endpoint-policies'
import type {
	CommunicationCall,
	CommunicationMethod,
	CommunicationPolicy,
	CommunicationPipelineOptions,
	CommunicationPolicyOverride,
	CommunicationPublish,
	CommunicationResult,
} from './pipeline-types'
import { publishMessage } from './transport'
import { waitForReceipt, waitForResult } from './transport'
import { waitForCoreRelayReceipt, waitForCoreRelayResult } from './core-relay'
import type { MessageDefinition, MessageEnvelope } from './transport'
import { sleep, toMessageMode, withCommunicationTimeout } from './pipeline-helpers'

export class CommunicationPipeline {
	constructor(
		private readonly adapter: PlatformAdapter,
		private readonly options: CommunicationPipelineOptions = {},
	) {}

	resolvePolicy(key: string, override: CommunicationPolicyOverride = {}): CommunicationPolicy {
		return mergePolicy(key, undefined, { ...this.options.defaultPolicy, ...override })
	}

	async call<T>(call: CommunicationCall<T>): Promise<CommunicationResult<T>> {
		const policy = this.resolvePolicy(call.key, call.policy)
		let lastError: unknown = null
		let attempts = 0

		for (const method of policy.methods) {
			if (method !== 'core-direct') {
				const queued = await this.tryQueueCall(call, policy, method, lastError)
				if (queued) return this.emit(queued)
				continue
			}

			for (let attempt = 0; attempt <= policy.retries; attempt++) {
				attempts++
				try {
					const value = await withCommunicationTimeout(call.execute, policy)
					return this.emit({
						ok: true,
						key: policy.key,
						method,
						attempts,
						queued: false,
						value,
					})
				} catch (error) {
					lastError = error
					if (!this.shouldRetry(error) || attempt === policy.retries) break
					await sleep(policy.retryDelayMs)
				}
			}
		}

		const result = this.emit<T>({
			ok: false,
			key: policy.key,
			method: policy.methods[0] ?? 'core-direct',
			attempts,
			queued: false,
			error:
				lastError ?? new AmberiteApiError(`No communication method available for ${policy.key}`),
		})

		if (policy.throwOnError) throw result.error
		return result
	}

	async callValue<T>(call: CommunicationCall<T>): Promise<T> {
		const result = await this.call(call)
		if (!result.ok) throw result.error
		return result.value as T
	}

	async publish<TPayload>(
		publish: CommunicationPublish<TPayload>,
	): Promise<CommunicationResult<MessageEnvelope<TPayload>>> {
		const policy = this.resolvePolicy(publish.key, publish.policy)
		let lastError: unknown = null
		let attempts = 0

		for (const method of policy.methods) {
			if (method === 'convex-relay' && (!policy.allowConvexRelay || !this.adapter.convexUrl))
				continue

			try {
				attempts++
				const envelope = await this.publishWithMethod(publish, policy, method)
				const resultValue = await this.maybeWaitForPublishResult(method, envelope.id, policy)
				return this.emit({
					ok: true,
					key: policy.key,
					method,
					attempts,
					queued: method === 'memory-queue' || method === 'persistent-queue',
					value: envelope,
					envelope,
					relayResult: resultValue ?? undefined,
				})
			} catch (error) {
				lastError = error
				if (!this.shouldRetry(error)) break
				await sleep(policy.retryDelayMs)
			}
		}

		const result = this.emit<MessageEnvelope<TPayload>>({
			ok: false,
			key: policy.key,
			method: policy.methods[0] ?? 'fire-and-forget',
			attempts,
			queued: false,
			error: lastError ?? new AmberiteApiError(`No publish method available for ${policy.key}`),
		})

		if (policy.throwOnError) throw result.error
		return result
	}

	private async tryQueueCall<T>(
		call: CommunicationCall<T>,
		policy: CommunicationPolicy,
		method: CommunicationMethod,
		lastError: unknown,
	): Promise<CommunicationResult<T> | null> {
		if (method !== 'memory-queue' && method !== 'persistent-queue') return null
		if (!this.adapter.queueStore || call.payload === undefined) return null

		const queued = {
			id: crypto.randomUUID(),
			createdAt: Date.now(),
			payload: {
				key: policy.key,
				payload: call.payload,
				lastError: lastError instanceof Error ? lastError.message : String(lastError ?? ''),
			},
		}

		await this.adapter.queueStore.push(policy.queueName, queued)
		return {
			ok: true,
			key: policy.key,
			method,
			attempts: 0,
			queued: true,
		}
	}

	private publishWithMethod<TPayload>(
		publish: CommunicationPublish<TPayload>,
		policy: CommunicationPolicy,
		method: CommunicationMethod,
	): Promise<MessageEnvelope<TPayload>> {
		const definition: MessageDefinition<TPayload> = {
			...publish.definition,
			mode: toMessageMode(method),
			ack: policy.ack,
			ttlMs: policy.ttlMs,
		}

		return publishMessage(this.adapter, { ...publish, definition, queueName: policy.queueName })
	}

	private async maybeWaitForPublishResult(
		method: CommunicationMethod,
		messageId: string,
		policy: CommunicationPolicy,
	): Promise<unknown | null> {
		if (!policy.waitForAck && !policy.waitForResult) return null
		if (method === 'core-relay') {
			if (policy.waitForResult)
				return await waitForCoreRelayResult(this.adapter, messageId, policy.timeoutMs)
			await waitForCoreRelayReceipt(this.adapter, messageId, policy.timeoutMs)
			return null
		}
		if (method === 'convex-relay') {
			if (policy.waitForResult)
				return await waitForResult(this.adapter, messageId, policy.timeoutMs)
			await waitForReceipt(this.adapter, messageId, policy.timeoutMs)
		}
		return null
	}

	private shouldRetry(error: unknown): boolean {
		return error instanceof NetworkError
	}

	private emit<T>(result: CommunicationResult<T>): CommunicationResult<T> {
		this.options.onResult?.(result)
		return result
	}
}
