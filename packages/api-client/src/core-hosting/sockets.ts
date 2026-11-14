import {
	type ClientMessage,
	core,
	type CoreEvent,
	type EngineEvent,
	type InstallJobSnapshot,
	type Topic,
} from '@amberite/contracts'
import mitt from 'mitt'

import type { AbstractModrinthClient } from '../core/abstract-client'
import { AbstractSyncClient, type SyncEmitterEvents } from '../core/abstract-sync'
import { AbstractWebSocketClient, type WebSocketConnection } from '../core/abstract-websocket'
import type { Archon } from '../modules/archon/types'

type WSEvent = Archon.Websocket.v0.WSEvent
type InstallProgressItem = Archon.Websocket.v0.InstallProgressItem

type WSEventMap = {
	[K in WSEvent as `${string}:${K['event']}`]: K
}

const NORMAL_CLOSURE = 1000

/** A Core has no sync stream: its servers' changes come over `CoreWebSocketClient`. */
export class CoreSyncClient extends AbstractSyncClient {
	protected emitter = mitt<SyncEmitterEvents>()

	async safeConnectServer(): Promise<void> {}

	disconnect(serverId: string): void {
		this.clearListeners(serverId)
	}

	disconnectAll(): void {}
}

/**
 * The hosting panel's per-server socket on a Core: a Core's `/events` socket subscribed to one
 * server's topics, each event passed on as the hosting API's event the panel listens for. A
 * Viewer's console subscription ends at once, so they get no console. Commands go to
 * `POST /servers/{id}/console`.
 *
 * The panel opens once it has the server's install progress. `installs` is subscribed first, so
 * the unfinished jobs it starts with have arrived when the power state authenticates the socket.
 */
export class CoreWebSocketClient extends AbstractWebSocketClient {
	protected emitter = mitt<WSEventMap>()

	constructor(private modrinth: AbstractModrinthClient) {
		super(modrinth)
	}

	connect(serverId: string, auth: Archon.Websocket.v0.WSAuth): Promise<void> {
		return this.open(serverId, auth.url, 0)
	}

	private open(serverId: string, url: string, reconnectAttempts: number): Promise<void> {
		this.closeConnection(serverId)
		return new Promise((resolve, reject) => {
			const ws = new WebSocket(url)
			const connection: WebSocketConnection = {
				serverId,
				socket: ws,
				authenticated: false,
				reconnectAttempts,
				isReconnecting: reconnectAttempts > 0,
			}
			this.connections.set(serverId, connection)
			const installs = new Map<string, InstallProgressItem>()
			const emitInstalls = () =>
				this.emit(serverId, { event: 'install-progress', items: [...installs.values()] })
			const current = () => this.connections.get(serverId) === connection

			ws.onopen = () => {
				const topics: Topic[] = [
					{ type: 'installs' },
					{ type: 'server', id: serverId },
					{ type: 'backups', id: serverId },
					{ type: 'console', id: serverId },
				]
				for (const topic of topics) {
					ws.send(JSON.stringify({ type: 'subscribe', topic } satisfies ClientMessage))
				}
			}

			ws.onmessage = (message) => {
				if (!current()) return
				const event = JSON.parse(message.data) as CoreEvent | EngineEvent
				if (event.type === 'install_job') {
					const job = event.payload
					if (job.instance_id !== serverId) return
					const item = toInstallItem(serverId, job)
					const finished = job.status !== 'queued' && job.status !== 'running'
					installs.set(job.job_id, item)
					if (connection.authenticated) {
						emitInstalls()
						if (finished) this.emit(serverId, toInstallResult(item))
					}
					if (finished) installs.delete(job.job_id)
					return
				}
				// The Core resends the unfinished jobs after a resync.
				if (event.type === 'resync') {
					installs.clear()
					if (connection.authenticated) emitInstalls()
					return
				}
				// Of this server's topics, `server` answers first: with the power state, or `ended`
				// without access.
				if (event.type === 'ended' && event.payload.topic.type === 'server') {
					connection.authenticated = false
					this.emit(serverId, { event: 'auth-incorrect' })
					reject(new Error(`No access to server ${serverId} on its Core`))
					this.closeConnection(serverId)
					return
				}
				if (event.type === 'power' && !connection.authenticated) {
					connection.authenticated = true
					connection.reconnectAttempts = 0
					connection.isReconnecting = false
					this.emit(serverId, { event: 'auth-ok' })
					emitInstalls()
					resolve()
				}
				for (const archon of toArchon(event)) this.emit(serverId, archon)
			}

			ws.onclose = (event) => {
				connection.authenticated = false
				reject(new Error(`The event socket for server ${serverId} closed (code: ${event.code})`))
				if (event.code !== NORMAL_CLOSURE && current()) this.scheduleReconnect(connection, url)
			}

			ws.onerror = () => {
				reject(new Error(`The event socket for server ${serverId} failed`))
			}
		})
	}

	private emit<E extends WSEvent>(serverId: string, event: E): void {
		this.emitter.emit(`${serverId}:${event.event}`, event)
	}

	disconnect(serverId: string): void {
		this.closeConnection(serverId)
		this.emitter.all.forEach((_handlers, type) => {
			if (type.toString().startsWith(`${serverId}:`)) this.emitter.all.delete(type)
		})
	}

	private closeConnection(serverId: string): void {
		const connection = this.connections.get(serverId)
		if (!connection) return
		clearTimeout(connection.reconnectTimer)
		this.connections.delete(serverId)
		if (connection.socket.readyState <= WebSocket.OPEN) {
			connection.socket.close(NORMAL_CLOSURE, 'Client disconnecting')
		}
	}

	closeSocket(serverId: string): void {
		this.closeConnection(serverId)
	}

	disconnectAll(): void {
		for (const serverId of this.connections.keys()) this.disconnect(serverId)
	}

	send(serverId: string, message: Archon.Websocket.v0.WSOutgoingMessage): void {
		if (message.event !== 'command') return
		void this.modrinth.archon.cores_v1
			.coreOf(serverId)
			.then(({ id }) =>
				this.modrinth
					.core(id)
					.call(core.SendCommand, { params: { id: serverId }, body: { command: message.cmd } }),
			)
			.catch((error: unknown) =>
				console.error(`[CoreSocket] Command not sent to ${serverId}:`, error),
			)
	}

	private scheduleReconnect(connection: WebSocketConnection, url: string): void {
		if (connection.reconnectAttempts >= this.MAX_RECONNECT_ATTEMPTS) {
			this.disconnect(connection.serverId)
			return
		}
		connection.isReconnecting = true
		connection.reconnectAttempts++
		connection.reconnectTimer = setTimeout(() => {
			if (this.connections.get(connection.serverId) !== connection) return
			this.open(connection.serverId, url, connection.reconnectAttempts).catch(() => {})
		}, this.getReconnectDelay(connection.reconnectAttempts))
	}
}

/**
 * A server's content installs are the engine's install jobs on its instance, which has its id.
 * A finished job reads as done (100) or failed (an error).
 */
function toInstallItem(serverId: string, job: InstallJobSnapshot): InstallProgressItem {
	const item: InstallProgressItem = {
		world_id: serverId,
		key: { type: 'local_modpack', filename: job.display?.title ?? '' },
		id: job.job_id,
		progress: null,
		error: null,
	}
	switch (job.status) {
		case 'queued':
		case 'running': {
			const progress = job.progress
			if (progress && progress.total > 0) {
				item.progress = Math.floor((progress.current / progress.total) * 100)
			}
			return item
		}
		case 'succeeded':
			return { ...item, progress: 100 }
		case 'failed':
		case 'interrupted':
		case 'canceled':
			return { ...item, error: job.error?.message ?? job.status }
	}
}

function toInstallResult(item: InstallProgressItem): WSEvent {
	return item.error === null
		? { event: 'installation-result', result: 'ok' }
		: { event: 'installation-result', result: 'err', reason: item.error }
}

function toArchon(event: CoreEvent | EngineEvent): WSEvent[] {
	switch (event.type) {
		case 'console':
			return [{ event: 'log', stream: 'stdout', message: event.payload.line }]
		case 'power': {
			const { state, uptime } = event.payload
			const power: WSEvent = { event: 'power-state', state }
			return uptime === null ? [power] : [power, { event: 'uptime', uptime }]
		}
		case 'backup': {
			const { server: _, ...progress } = event.payload
			return [{ event: 'backup-progress', ...progress }]
		}
		default:
			return []
	}
}
