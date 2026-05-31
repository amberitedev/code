import type { PlatformAdapter } from './adapter'
import { verifyCoreConnection, type ConnectionState, type ConnectionStatus } from './connection'

type StateChangeListener = (state: ConnectionState, status: ConnectionStatus) => void
type StatusListener = (status: ConnectionStatus) => void

const CHECK_INTERVAL_MS = 10_000

export class CoreConnectionMonitor {
	private state: ConnectionState = 'unknown'
	private status: ConnectionStatus | null = null
	private stateListeners: Array<StateChangeListener> = []
	private statusListeners: Array<StatusListener> = []
	private timer: ReturnType<typeof setInterval> | null = null

	constructor(
		private adapter: PlatformAdapter,
		private knownCoreId?: string | null,
		_senderId?: string,
	) {}

	get currentState(): ConnectionState {
		return this.state
	}

	get currentStatus(): ConnectionStatus | null {
		return this.status
	}

	onStateChange(cb: StateChangeListener): () => void {
		this.stateListeners.push(cb)
		return () => removeListener(this.stateListeners, cb)
	}

	onStatus(cb: StatusListener): () => void {
		this.statusListeners.push(cb)
		return () => removeListener(this.statusListeners, cb)
	}

	async start(): Promise<void> {
		this.setState('connecting')
		this.timer = setInterval(() => this.checkNow().catch(() => {}), CHECK_INTERVAL_MS)
		await this.checkNow()
	}

	stop(): void {
		if (this.timer) clearInterval(this.timer)
		this.timer = null
	}

	async checkNow(): Promise<ConnectionStatus> {
		const next = await verifyCoreConnection(this.adapter, { knownCoreId: this.knownCoreId })
		this.status = next
		this.setState(next.state, next)
		for (const cb of this.statusListeners) cb(next)
		return next
	}

	private setState(next: ConnectionState, status = this.status): void {
		if (this.state === next) return
		this.state = next
		if (status) for (const cb of this.stateListeners) cb(next, status)
	}
}

function removeListener<T>(listeners: T[], listener: T): void {
	const idx = listeners.indexOf(listener)
	if (idx !== -1) listeners.splice(idx, 1)
}

export type { ConnectionState, ConnectionStatus }
