import type { CoreClientAdapter } from './adapter'
import { type ConnectionState, type ConnectionStatus, verifyCoreConnection } from './connection'

type StateChangeListener = (state: ConnectionState, status: ConnectionStatus) => void
type StatusListener = (status: ConnectionStatus) => void

export class CoreConnectionMonitor {
	private state: ConnectionState = 'unknown'
	private status: ConnectionStatus | null = null
	private stateListeners: Array<StateChangeListener> = []
	private statusListeners: Array<StatusListener> = []

	constructor(
		private adapter: CoreClientAdapter,
		private knownCoreId?: string | null,
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
		await this.checkNow()
	}

	stop(): void {}

	async checkNow(coreUrl?: string): Promise<ConnectionStatus> {
		const knownCoreId = this.knownCoreId ?? (await this.adapter.getConnectedCoreId?.())
		const next = await verifyCoreConnection(this.adapter, { coreUrl, knownCoreId })
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
