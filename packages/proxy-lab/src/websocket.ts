import type { IncomingMessage } from 'node:http'
import type { Duplex } from 'node:stream'

import type { Archon } from '../../api-client/src/modules/archon/types.ts'
import { WebSocket, WebSocketServer } from 'ws'

import { PERSONAS, SERVER_ID } from './fixtures.ts'

export class WebSocketHub {
	private readonly server = new WebSocketServer({ noServer: true })
	private readonly connections = new Set<WebSocket>()
	private readonly hostingConnections = new Set<WebSocket>()
	private readonly getPowerState: () => Archon.Websocket.v0.PowerState

	constructor(getPowerState: () => Archon.Websocket.v0.PowerState) {
		this.getPowerState = getPowerState
	}

	handleUpgrade(request: IncomingMessage, socket: Duplex, head: Buffer): void {
		const url = new URL(request.url ?? '/', 'http://proxy-lab.invalid')
		const isHosting = url.pathname === `/ws/${SERVER_ID}`
		const isFriends = url.pathname === '/_internal/launcher_socket'
		if (!isHosting && !isFriends) {
			socket.write('HTTP/1.1 404 Not Found\r\nConnection: close\r\n\r\n')
			socket.destroy()
			return
		}

		if (isFriends && !isFixtureToken(url.searchParams.get('code'))) {
			socket.write('HTTP/1.1 401 Unauthorized\r\nConnection: close\r\n\r\n')
			socket.destroy()
			return
		}

		this.server.handleUpgrade(request, socket, head, (connection) => {
			this.connections.add(connection)
			connection.once('close', () => this.connections.delete(connection))
			if (isFriends) {
				connection.on('error', () => undefined)
				return
			}
			this.handleHostingConnection(connection)
		})
	}

	broadcastHosting(event: Archon.Websocket.v0.WSEvent): void {
		const encoded = JSON.stringify(event)
		for (const connection of this.hostingConnections) {
			if (connection.readyState === WebSocket.OPEN) connection.send(encoded)
		}
	}

	close(): Promise<void> {
		for (const connection of this.connections) connection.close(1001, 'Proxy lab stopped')
		return new Promise((resolve) => this.server.close(() => resolve()))
	}

	private handleHostingConnection(connection: WebSocket): void {
		let authenticated = false
		let stateInterval: ReturnType<typeof setInterval> | undefined
		const authTimeout = setTimeout(() => connection.close(1008, 'Authentication required'), 5_000)
		connection.on('error', () => undefined)
		connection.on('close', () => {
			clearTimeout(authTimeout)
			if (stateInterval) clearInterval(stateInterval)
			this.hostingConnections.delete(connection)
		})
		connection.on('message', (raw, isBinary) => {
			if (isBinary) {
				connection.close(1003, 'Text messages only')
				return
			}
			let message: unknown
			try {
				message = JSON.parse(raw.toString()) as unknown
			} catch {
				connection.close(1007, 'Invalid JSON')
				return
			}

			if (!authenticated) {
				if (!isHostingAuth(message)) {
					connection.send(JSON.stringify({ event: 'auth-incorrect' }))
					connection.close(1008, 'Invalid fixture token')
					return
				}
				authenticated = true
				clearTimeout(authTimeout)
				this.hostingConnections.add(connection)
				this.sendInitialHostingState(connection)
				stateInterval = setInterval(() => this.sendRuntimeState(connection), 2_000)
				stateInterval.unref()
				return
			}

			if (isCommand(message)) {
				this.broadcastHosting({
					event: 'log',
					stream: 'stdout',
					message: `[Proxy Lab] > ${message.cmd}\n`,
				})
				this.broadcastHosting({
					event: 'log',
					stream: 'stdout',
					message: '[Server thread/INFO]: Command accepted by the local simulation.\n',
				})
			}
		})
	}

	private sendInitialHostingState(connection: WebSocket): void {
		const events: Archon.Websocket.v0.WSEvent[] = [
			{ event: 'auth-ok' },
			{ event: 'install-progress', items: [] },
			{ event: 'power-state', state: this.getPowerState() },
			{
				event: 'log',
				stream: 'stdout',
				message: '[Proxy Lab] Local console connected.\n',
			},
		]
		for (const event of events) connection.send(JSON.stringify(event))
		this.sendRuntimeState(connection)
	}

	private sendRuntimeState(connection: WebSocket): void {
		if (connection.readyState !== WebSocket.OPEN) return
		const running = this.getPowerState() === 'running'
		const events: Archon.Websocket.v0.WSEvent[] = [
			{
				event: 'stats',
				cpu_percent: running ? 18.5 : 0,
				ram_usage_bytes: running ? 1_342_177_280 : 0,
				ram_total_bytes: 4_294_967_296,
				storage_usage_bytes: 5_368_709_120,
				storage_total_bytes: 21_474_836_480,
				net_tx_bytes: 42_000,
				net_rx_bytes: 88_000,
			},
			{ event: 'uptime', uptime: running ? 7_200 : 0 },
		]
		for (const event of events) connection.send(JSON.stringify(event))
	}
}

function isHostingAuth(value: unknown): boolean {
	return (
		typeof value === 'object' &&
		value !== null &&
		'event' in value &&
		value.event === 'auth' &&
		'jwt' in value &&
		value.jwt === 'proxy-lab-websocket-token'
	)
}

function isCommand(value: unknown): value is { event: 'command'; cmd: string } {
	return (
		typeof value === 'object' &&
		value !== null &&
		'event' in value &&
		value.event === 'command' &&
		'cmd' in value &&
		typeof value.cmd === 'string'
	)
}

function isFixtureToken(token: string | null): boolean {
	return Object.values(PERSONAS).some((persona) => persona.token === token)
}
