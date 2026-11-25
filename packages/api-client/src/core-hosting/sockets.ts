import {
	core,
	type InstallJobSnapshot,
	type InstancePayload,
	type Topic,
} from '@amberite/contracts'
import mitt from 'mitt'

import type { AbstractModrinthClient, CoreStreamEvent } from '../core/abstract-client'
import { AbstractSyncClient, type SyncEmitterEvents } from '../core/abstract-sync'
import { AbstractWebSocketClient, type WebSocketStatus } from '../core/abstract-websocket'
import type { Archon } from '../modules/archon/types'

type WSEvent = Archon.Websocket.v0.WSEvent
type InstallProgressItem = Archon.Websocket.v0.InstallProgressItem

type WSEventMap = {
	[K in WSEvent as `${string}:${K['event']}`]: K
}

/**
 * A Core has no sync stream: its servers' changes come over `CoreWebSocketClient`, which passes
 * them on with {@link CoreSyncClient.changed}.
 */
export class CoreSyncClient extends AbstractSyncClient {
	protected emitter = mitt<SyncEmitterEvents>()

	async safeConnectServer(): Promise<void> {}

	/**
	 * The server's content or share changed, or an install let it go: the panel fetches its
	 * details again, which say whether it installs and has changes to share. A transfer's end is
	 * the sync event the panel does that on.
	 */
	changed(serverId: string): void {
		this.emitSyncEvent(serverId, { type: 'server.transfer.done', target_node: '' })
	}

	disconnect(serverId: string): void {
		this.clearListeners(serverId)
	}

	disconnectAll(): void {}
}

/** What the panel hears of one server: a view of its Core's one event socket. */
type View = {
	stops: (() => void)[]
	authenticated: boolean
	installs: Map<string, InstallProgressItem>
}

/**
 * The hosting panel's per-server socket on a Core: a view of the Core's one event socket
 * (`client.coreEvents`), subscribed to one server's topics, each event passed on as the hosting
 * API's event the panel listens for. Commands go to `POST /servers/{id}/console`.
 *
 * The panel opens once it has the server's install progress. `installs` is subscribed first, so
 * the unfinished jobs it starts with have arrived when the power state authenticates the view.
 * A change to the server's content (its `instance` topic) reads as an install result, the one
 * event on which the panel fetches the content again.
 */
export class CoreWebSocketClient extends AbstractWebSocketClient {
	protected emitter = mitt<WSEventMap>()
	private views = new Map<string, View>()

	constructor(private modrinth: AbstractModrinthClient) {
		super(modrinth)
	}

	async connect(serverId: string): Promise<void> {
		this.closeConnection(serverId)
		const { id: coreId } = await this.modrinth.cores.coreOf(serverId)
		const events = this.modrinth.coreEvents(coreId)
		const view: View = { stops: [], authenticated: false, installs: new Map() }
		this.views.set(serverId, view)
		const emitInstalls = () =>
			this.emit(serverId, { event: 'install-progress', items: [...view.installs.values()] })
		return new Promise((resolve) => {
			view.stops.push(
				events.onStatus((open) => {
					if (!open) view.authenticated = false
				}),
				events.onAny((event) => {
					if (event.type === 'install_job') {
						const job = event.payload
						if (job.instance_id !== serverId) return
						const item = toInstallItem(serverId, job)
						const finished = job.status !== 'queued' && job.status !== 'running'
						view.installs.set(job.job_id, item)
						if (view.authenticated) {
							emitInstalls()
							if (finished) this.emit(serverId, toInstallResult(item))
						}
						if (finished) view.installs.delete(job.job_id)
						return
					}
					// The Core resends the unfinished jobs after a resync.
					if (event.type === 'resync') {
						view.installs.clear()
						if (view.authenticated) emitInstalls()
						return
					}
					if (!concerns(event, serverId)) return
					// Of this server's topics, `server` answers first, with the power state. A server
					// that does not exist has none, and its routes answer `not_found`.
					if (event.type === 'power_state' && !view.authenticated) {
						view.authenticated = true
						this.emit(serverId, { event: 'auth-ok' })
						emitInstalls()
						resolve()
					}
					if (
						(event.type === 'instance' && changesServer(event.payload)) ||
						event.type === 'server_changed'
					) {
						const { sync } = this.modrinth.archon
						if (sync instanceof CoreSyncClient) sync.changed(serverId)
					}
					for (const archon of toArchon(event)) this.emit(serverId, archon)
				}),
			)
			for (const topic of topics(serverId)) view.stops.push(events.subscribe(topic))
		})
	}

	override getStatus(serverId: string): WebSocketStatus | null {
		const view = this.views.get(serverId)
		if (!view) return null
		return { connected: view.authenticated, reconnecting: false, reconnectAttempts: 0 }
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
		const view = this.views.get(serverId)
		if (!view) return
		this.views.delete(serverId)
		for (const stop of view.stops) stop()
	}

	closeSocket(serverId: string): void {
		this.closeConnection(serverId)
	}

	disconnectAll(): void {
		for (const serverId of [...this.views.keys()]) this.disconnect(serverId)
	}

	send(serverId: string, message: Archon.Websocket.v0.WSOutgoingMessage): void {
		if (message.event !== 'command') return
		void this.modrinth.cores
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
}

/** One server's topics on its Core's socket. */
function topics(serverId: string): Topic[] {
	return [
		{ type: 'installs' },
		{ type: 'server', id: serverId },
		{ type: 'backups', id: serverId },
		{ type: 'console', id: serverId },
		{ type: 'instance', id: serverId },
	]
}

/** Whether `event`, from a socket every server's view shares, is about `serverId`. */
function concerns(event: CoreStreamEvent, serverId: string): boolean {
	switch (event.type) {
		case 'power_state':
		case 'uptime':
		case 'backup_progress':
		case 'server_changed':
			return event.payload.server === serverId
		case 'log':
		case 'instance':
		case 'process':
			return event.payload.instance_id === serverId
		default:
			return false
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

/** Whether the server's details may read differently after `payload`. */
function changesServer({ event }: InstancePayload) {
	return (
		event === 'edited' || event === 'content_install_finished' || event === 'content_install_failed'
	)
}

function toInstallResult(item: InstallProgressItem): WSEvent {
	return item.error === null
		? { event: 'installation-result', result: 'ok' }
		: { event: 'installation-result', result: 'err', reason: item.error }
}

function toArchon(event: CoreStreamEvent): WSEvent[] {
	switch (event.type) {
		case 'log':
			return event.payload.type === 'legacy'
				? [{ event: 'log', stream: 'stdout', message: event.payload.message }]
				: []
		case 'power_state':
			return [{ event: 'power-state', state: event.payload.state }]
		case 'uptime':
			return [{ event: 'uptime', uptime: event.payload.uptime }]
		case 'backup_progress': {
			const { server: _, ...progress } = event.payload
			return [{ event: 'backup-progress', ...progress }]
		}
		case 'instance':
			switch (event.payload.event) {
				case 'edited':
				case 'content_install_finished':
					return [{ event: 'installation-result', result: 'ok' }]
				case 'content_install_failed':
					return [{ event: 'installation-result', result: 'err', reason: event.payload.message }]
				default:
					return []
			}
		default:
			return []
	}
}
