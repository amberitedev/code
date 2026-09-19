import { randomUUID } from 'node:crypto'
import { createServer } from 'node:http'
import { join } from 'node:path'

import { HOST, PERSONAS, PORT } from './fixtures.ts'
import { handleHosting } from './hosting.ts'
import { empty, HttpError, json } from './http.ts'
import { handleIdentity, proxyPublicCatalog } from './identity.ts'
import { handleSharedInstances } from './shared.ts'
import { createStateStore } from './state.ts'
import { redactHeaders, TraceWriter } from './trace.ts'
import type { Persona } from './types.ts'
import { WebSocketHub } from './websocket.ts'

export type ProxyLabServerOptions = {
	host?: string
	port?: number
	dataDir: string
	publicCatalog?: boolean
}

export type RunningProxyLabServer = {
	origin: string
	close(): Promise<void>
}

const EMPTY_ZIP = Buffer.from('UEsFBgAAAAAAAAAAAAAAAAAAAAAAAA==', 'base64')

export async function startProxyLabServer(
	options: ProxyLabServerOptions,
): Promise<RunningProxyLabServer> {
	const host = options.host ?? HOST
	const port = options.port ?? PORT
	const publicCatalog = options.publicCatalog ?? false
	const store = await createStateStore(options.dataDir)
	const trace = new TraceWriter(join(options.dataDir, 'trace.jsonl'))
	const sockets = new WebSocketHub(() => store.read().archon.power_state)

	const server = createServer(async (request, response) => {
		const started = performance.now()
		const requestId = randomUUID()
		const url = new URL(request.url ?? '/', `http://${request.headers.host ?? `${host}:${port}`}`)
		const persona = getPersona(request.headers.authorization)

		try {
			if (request.method === 'OPTIONS') {
				empty(response)
				return
			}
			if (request.method === 'GET' && url.pathname === '/__proxy_lab/status') {
				const state = store.read()
				json(response, 200, {
					service: 'modrinth-proxy-lab',
					public_catalog: publicCatalog,
					server_id: state.archon.server_v0.server_id,
					power_state: state.archon.power_state,
					shared_instance_ids: state.shared_instances.map((item) => item.id),
				})
				return
			}
			if (request.method === 'GET' && url.pathname === '/fixtures/proxy-lab-empty.jar') {
				response.writeHead(200, {
					'access-control-allow-origin': 'http://localhost:1420',
					'cache-control': 'no-store',
					'content-length': EMPTY_ZIP.length,
					'content-type': 'application/java-archive',
				})
				response.end(EMPTY_ZIP)
				return
			}
			if (
				request.method === 'GET' &&
				url.pathname === '/node/modrinth/v0/fs/download' &&
				['/server-icon.png', '/server-icon-original.png'].includes(
					url.searchParams.get('path') ?? '',
				)
			) {
				if (!persona) throw new HttpError(401, 'Use a proxy-lab fixture token')
				empty(response, 404)
				return
			}

			const context = { request, response, url, persona, store, sockets, publicCatalog }
			if (await handleSharedInstances(context)) return
			if (await handleHosting(context)) return
			if (await handleIdentity(context)) return
			if (publicCatalog && (await proxyPublicCatalog(request, response, url))) return

			json(response, 501, {
				error: 'proxy_lab_route_not_implemented',
				message: `${request.method ?? 'GET'} ${url.pathname} is outside the proxy-lab contract`,
				docs: 'packages/proxy-lab/README.md#implemented-contract',
			})
		} catch (error) {
			const status = error instanceof HttpError ? error.status : 500
			json(response, status, {
				error: status === 500 ? 'proxy_lab_internal_error' : 'proxy_lab_request_error',
				message: error instanceof Error ? error.message : 'Unknown proxy-lab error',
				request_id: requestId,
			})
			if (status === 500) console.error(error)
		} finally {
			await trace.write({
				time: new Date().toISOString(),
				request_id: requestId,
				method: request.method ?? 'GET',
				path: `${url.pathname}${url.search}`,
				status: response.statusCode,
				persona: persona?.id ?? null,
				request_headers: redactHeaders(request.headers),
				duration_ms: Math.round((performance.now() - started) * 100) / 100,
			})
		}
	})

	server.on('upgrade', (request, socket, head) => sockets.handleUpgrade(request, socket, head))
	await new Promise<void>((resolve, reject) => {
		server.once('error', reject)
		server.listen(port, host, () => {
			server.off('error', reject)
			resolve()
		})
	})
	const address = server.address()
	if (!address || typeof address === 'string') throw new Error('Proxy lab did not bind a TCP port')

	return {
		origin: `http://${host}:${address.port}`,
		async close(): Promise<void> {
			await sockets.close()
			await new Promise<void>((resolve, reject) =>
				server.close((error) => (error ? reject(error) : resolve())),
			)
			await trace.close()
		},
	}
}

function getPersona(authorization: string | undefined): Persona | null {
	if (!authorization) return null
	const token = authorization.replace(/^Bearer\s+/i, '')
	if (token === 'proxy-lab-node-token') return PERSONAS.owner
	return Object.values(PERSONAS).find((persona) => persona.token === token) ?? null
}
