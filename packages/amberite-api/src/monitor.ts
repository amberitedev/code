import type { PlatformAdapter } from './adapter'
import { corePresence } from './transport'

export type ConnectionState = 'connecting' | 'online-direct' | 'online-relay' | 'offline'

type StateChangeListener = (state: ConnectionState) => void

const HEALTH_PATH = '/health'
const PING_INTERVAL_MS = 10_000
const OFFLINE_THRESHOLD_MS = 30_000
const HEALTH_TIMEOUT_MS = 5_000

export class CoreConnectionMonitor {
	private state: ConnectionState = 'connecting'
	private listeners: Array<StateChangeListener> = []
	private timer: ReturnType<typeof setInterval> | null = null

	constructor(
		private adapter: PlatformAdapter,
		private coreId: string,
		private _senderId: string,
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
		this.timer = setInterval(() => this.tick(), PING_INTERVAL_MS)
		await this.tick()
	}

	stop(): void {
		if (this.timer) {
			clearInterval(this.timer)
			this.timer = null
		}
	}

	private async tick(): Promise<void> {
		const directOk = await this.directHealthOk()
		if (directOk) {
			this.setState('online-direct')
			return
		}

		try {
			const presence = (await corePresence(this.adapter, this.coreId)) as {
				lastSeenAt?: number
			} | null
			if (presence?.lastSeenAt && Date.now() - presence.lastSeenAt < OFFLINE_THRESHOLD_MS) {
				this.setState('online-relay')
				return
			}
		} catch {
			// Convex presence is best-effort for monitoring.
		}

		this.setState('offline')
	}

	private async directHealthOk(): Promise<boolean> {
		const coreUrl = await this.adapter.getCoreUrl()
		if (!coreUrl) return false
		const controller = new AbortController()
		const timeout = setTimeout(() => controller.abort(), HEALTH_TIMEOUT_MS)
		try {
			const res = await this.adapter.fetchFn(`${coreUrl}${HEALTH_PATH}`, {
				signal: controller.signal,
			})
			return res.ok
		} catch {
			return false
		} finally {
			clearTimeout(timeout)
		}
	}

	private setState(next: ConnectionState): void {
		if (this.state === next) return
		this.state = next
		for (const cb of this.listeners) cb(next)
	}
}
