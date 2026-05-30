import type { AckPolicy, MessageDefinition, MessageEnvelope, MessageMode } from './transport'

export type CommunicationSurface = 'core' | 'convex' | 'client' | 'local'

export type CommunicationMethod =
	| 'core-direct'
	| 'core-relay'
	| 'convex-relay'
	| 'memory-queue'
	| 'persistent-queue'
	| 'fire-and-forget'

export type CommunicationReliability = 'volatile' | 'interactive' | 'durable' | 'critical'
export type CommunicationAuthMode = 'none' | 'optional' | 'required' | 'dev-bypass'

export interface CommunicationNode {
	id: string
	kind: 'desktop-app' | 'core' | 'convex' | 'browser' | 'unknown'
	label?: string
	url?: string
}

export interface CommunicationPolicy {
	key: string
	surface: CommunicationSurface
	methods: CommunicationMethod[]
	timeoutMs: number
	retries: number
	retryDelayMs: number
	ttlMs: number
	ack: AckPolicy
	waitForAck: boolean
	waitForResult: boolean
	reliability: CommunicationReliability
	auth: CommunicationAuthMode
	queueName: string
	throwOnError: boolean
	allowConvexRelay: boolean
}

export type CommunicationPolicyOverride = Partial<Omit<CommunicationPolicy, 'key'>> & {
	key?: string
}

export interface CommunicationCall<T> {
	key: string
	surface?: CommunicationSurface
	execute: (signal: AbortSignal, policy: CommunicationPolicy) => Promise<T>
	policy?: CommunicationPolicyOverride
	payload?: unknown
}

export interface CommunicationPublish<TPayload = unknown> {
	key: string
	definition: MessageDefinition<TPayload>
	senderId: string
	recipientId: string
	payload: TPayload
	policy?: CommunicationPolicyOverride
}

export interface CommunicationResult<T = unknown> {
	ok: boolean
	key: string
	method: CommunicationMethod
	attempts: number
	queued: boolean
	value?: T
	error?: unknown
	envelope?: MessageEnvelope
	relayResult?: unknown
}

export interface CommunicationPipelineOptions {
	defaultPolicy?: CommunicationPolicyOverride
	onResult?: (result: CommunicationResult) => void
}

export type EndpointPolicyMap = Record<string, CommunicationPolicyOverride>
export type { MessageMode }
