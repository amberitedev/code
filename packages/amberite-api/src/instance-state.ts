import type { PlatformAdapter } from './adapter'
import { CoreApiClient, type CoreEventStream } from './client'
import type { ConnectionState } from './monitor'
import { CoreConnectionMonitor } from './monitor'
import type {
	CoreCreateInstanceBody,
	CoreInstance,
	CoreInstanceEvent,
	CoreInstanceSummary,
} from './types'

export type CoreInstanceStateSnapshot = {
	instances: CoreInstanceSummary[]
	connectionState: ConnectionState
	isRefreshing: boolean
	error: unknown | null
}

type SnapshotListener = (snapshot: CoreInstanceStateSnapshot) => void

export class CoreInstanceStateManager {
	readonly client: CoreApiClient
	private instances = new Map<string, CoreInstanceSummary>()
	private listeners: SnapshotListener[] = []
	private monitor: CoreConnectionMonitor | null = null
	private events: CoreEventStream | null = null
	private reconnectTimer: ReturnType<typeof setTimeout> | null = null
	private connectionState: ConnectionState = 'connecting'
	private isRefreshing = false
	private error: unknown | null = null

	constructor(private readonly adapter: PlatformAdapter) {
		this.client = new CoreApiClient(adapter)
	}

	get snapshot(): CoreInstanceStateSnapshot {
		return {
			instances: [...this.instances.values()],
			connectionState: this.connectionState,
			isRefreshing: this.isRefreshing,
			error: this.error,
		}
	}

	subscribe(listener: SnapshotListener): () => void {
		this.listeners.push(listener)
		listener(this.snapshot)
		return () => removeListener(this.listeners, listener)
	}

	async start(): Promise<void> {
		if (!this.monitor) {
			this.monitor = new CoreConnectionMonitor(this.adapter, '', '')
			this.monitor.onStateChange((state) => {
				this.connectionState = state
				this.emit()
				if (state === 'online-direct') {
					void this.refresh().catch(() => {})
					void this.connectEvents().catch(() => {})
				} else {
					this.disconnectEvents()
				}
			})
		}
		await this.monitor.start()
		await this.refresh().catch(() => {})
		await this.connectEvents().catch(() => {})
	}

	stop(): void {
		this.monitor?.stop()
		this.monitor = null
		this.disconnectEvents()
		if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
		this.reconnectTimer = null
	}

	async refresh(): Promise<CoreInstanceSummary[]> {
		this.isRefreshing = true
		this.error = null
		this.emit()
		try {
			const instances = await this.client.listInstances()
			this.instances = new Map(instances.map((instance) => [instance.id, instance]))
			this.error = null
			return instances
		} catch (error) {
			this.error = error
			throw error
		} finally {
			this.isRefreshing = false
			this.emit()
		}
	}

	get(id: string | null | undefined): CoreInstanceSummary | null {
		if (!id) return null
		return this.instances.get(id) ?? null
	}

	async create(body: CoreCreateInstanceBody): Promise<CoreInstanceSummary> {
		const instance = await this.client.createInstance(body)
		const summary = toSummary(instance)
		this.instances.set(summary.id, summary)
		this.emit()
		return summary
	}

	async delete(id: string): Promise<void> {
		await this.client.deleteInstance(id)
		this.instances.delete(id)
		this.emit()
	}

	async startInstance(id: string): Promise<void> {
		await this.client.start(id)
		this.patch(id, { status: 'starting' })
	}

	async stopInstance(id: string): Promise<void> {
		await this.client.stop(id)
		this.patch(id, { status: 'stopping' })
	}

	async restartInstance(id: string): Promise<void> {
		await this.client.restart(id)
		this.patch(id, { status: 'starting' })
	}

	private async connectEvents(): Promise<void> {
		if (this.events || this.connectionState !== 'online-direct') return
		try {
			this.events = await this.client.openEvents()
			this.events.onEvent((event) => this.applyEvent(event))
			this.events.onError((error) => {
				this.error = error
				this.emit()
			})
			this.events.onClose(() => {
				this.events = null
				if (this.connectionState === 'online-direct') this.scheduleEventReconnect()
			})
		} catch (error) {
			this.error = error
			this.emit()
			this.scheduleEventReconnect()
		}
	}

	private disconnectEvents(): void {
		this.events?.close()
		this.events = null
		if (this.reconnectTimer) clearTimeout(this.reconnectTimer)
		this.reconnectTimer = null
	}

	private scheduleEventReconnect(): void {
		if (this.reconnectTimer) return
		this.reconnectTimer = setTimeout(() => {
			this.reconnectTimer = null
			void this.connectEvents()
		}, 3000)
	}

	private applyEvent(event: CoreInstanceEvent): void {
		switch (event.type) {
			case 'instance_created':
			case 'instance_updated':
				this.instances.set(event.instance.id, event.instance)
				break
			case 'instance_deleted':
				this.instances.delete(event.instance_id)
				break
			case 'status_changed':
				this.patch(event.instance_id, { status: event.status }, false)
				break
			case 'install_status_changed':
				this.patch(event.instance_id, { install_status: event.install_status }, false)
				break
		}
		this.emit()
	}

	private patch(id: string, patch: Partial<CoreInstanceSummary>, emit = true): void {
		const instance = this.instances.get(id)
		if (!instance) return
		this.instances.set(id, { ...instance, ...patch })
		if (emit) this.emit()
	}

	private emit(): void {
		const snapshot = this.snapshot
		for (const listener of this.listeners) listener(snapshot)
	}
}

function toSummary(instance: CoreInstance | CoreInstanceSummary): CoreInstanceSummary {
	return {
		id: instance.id,
		name: instance.name,
		game_version: instance.game_version,
		loader: instance.loader,
		loader_version: instance.loader_version,
		port: instance.port,
		memory: instance.memory,
		install_status: instance.install_status,
		status: instance.status,
		created_at: instance.created_at,
		updated_at: instance.updated_at,
	}
}

function removeListener<T>(listeners: T[], listener: T): void {
	const index = listeners.indexOf(listener)
	if (index !== -1) listeners.splice(index, 1)
}
