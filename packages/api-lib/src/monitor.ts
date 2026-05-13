/**
 * CoreConnectionMonitor tracks the four states of connectivity to Core:
 * connecting, online-direct, online-relay, offline.
 *
 * State is driven by direct HTTP reachability and relay acknowledgment timing,
 * not by the last_seen heartbeat timestamp (which is UX-only).
 */

import type { PlatformAdapter } from './adapter'
import type { CoreCallContext } from './context'
import { publishMessage, waitForReceipt, subscribeToMessages } from './transport'
import { CoreOfflineError } from './errors'

export type ConnectionState = 'connecting' | 'online-direct' | 'online-relay' | 'offline'

type StateChangeListener = (state: ConnectionState) => void

const HEALTH_PATH = '/health'
const PING_INTERVAL_MS = 10_000
const OFFLINE_THRESHOLD_MS = 30_000

export class CoreConnectionMonitor {
	private state: ConnectionState = 'connecting'
	private listeners: Array<StateChangeListener> = []
	private timer: ReturnType<typeof setInterval> | null = null
	private unsub: { unsubscribe: () => void } | null = null
	private lastRelayAck = 0

	constructor(
		private adapter: PlatformAdapter,
		private coreId: string,
		private senderId: string,
	) {}

	get currentState(): ConnectionState {
		return this.state
	}

	onStateChange(cb: StateChangeListener): () => void {
		this.listeners.push(cb)
		return () => {
			const idx = this.listeners.indexOf(cb)
			if (idx !== -1) this.listeners.splice(idx, 1)
		}
	}

	async start(): Promise<void> {
		this.setState('connecting')
		this.unsub = subscribeToMessages(
			this.adapter.supabase,
			this.coreId,
			'core-to-client',
			async (msg) => {
				this.lastRelayAck = Date.now()
				if (this.state === 'offline') {
					this.setState('online-relay')
				}
			},
		)
		this.timer = setInterval(() => this.tick(), PING_INTERVAL_MS)
		await this.tick()
	}

	stop(): void {
		if (this.timer) {
			clearInterval(this.timer)
			this.timer = null
		}
		if (this.unsub) {
			this.unsub.unsubscribe()
			this.unsub = null
		}
	}

	private async tick(): Promise<void> {
		const coreUrl = await this.adapter.getCoreUrl()
		const token = await this.adapter.getLocalCoreToken()

		let directOk = false
		if (coreUrl) {
			try {
				const ctx: CoreCallContext = {
					baseUrl: coreUrl,
					token,
					fetchFn: this.adapter.fetchFn,
				}
				const res = await this.adapter.fetchFn(`${ctx.baseUrl}${HEALTH_PATH}`, {
					headers: token ? { Authorization: `Bearer ${token}` } : {},
				})
				directOk = res.ok
			} catch {
				directOk = false
			}
		}

		if (directOk) {
			this.setState('online-direct')
			return
		}

		const relayAlive = Date.now() - this.lastRelayAck < OFFLINE_THRESHOLD_MS
		if (this.state === 'online-direct' || this.state === 'connecting') {
			if (relayAlive) {
				this.setState('online-relay')
			} else {
				this.setState('offline')
			}
		} else if (this.state === 'online-relay' && !relayAlive) {
			this.setState('offline')
		}
	}

	private setState(next: ConnectionState): void {
		if (this.state === next) return
		this.state = next
		for (const cb of this.listeners) cb(next)
	}
}
