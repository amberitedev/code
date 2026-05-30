/**
 * useCoreMessage — typed Amberite message publishing through the shared pipeline.
 *
 * This is for async communication and relay traffic, not normal CRUD endpoints.
 * Every publish uses CommunicationPipeline so direct/Core relay/Convex relay/queue
 * behaviour is controlled by the same policy system as CoreApiClient calls.
 */
import type {
	CommunicationPolicyOverride,
	CommunicationResult,
	MessageDefinition,
	MessageEnvelope,
} from '@amberite/amberite-api'
import { ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'

export interface CoreMessageOptions<TPayload> {
	key: string
	definition: MessageDefinition<TPayload>
	senderId: string
	recipientId: string
	payload: TPayload
	policy?: CommunicationPolicyOverride
}

export function useCoreMessage<TPayload = unknown>() {
	const client = useCoreClient()
	const sending = ref(false)
	const error = ref<Error | null>(null)
	const lastResult = ref<CommunicationResult<MessageEnvelope<TPayload>> | null>(null)

	async function publish(
		options: CoreMessageOptions<TPayload>,
	): Promise<CommunicationResult<MessageEnvelope<TPayload>> | null> {
		sending.value = true
		error.value = null
		try {
			const result = await client.pipeline.publish(options)
			lastResult.value = result
			return result
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			return null
		} finally {
			sending.value = false
		}
	}

	return { sending, error, lastResult, publish }
}
