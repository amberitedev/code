import { afterEach, describe, expect, it, vi } from 'vitest'

import type { PlatformAdapter } from '../adapter'
import { CoreInstanceStateManager } from '../instance-state'
import type { CoreInstanceEvent, CoreInstanceSummary } from '../types'

describe('CoreInstanceStateManager', () => {
	let manager: CoreInstanceStateManager | null = null

	afterEach(() => {
		manager?.stop()
		manager = null
		vi.useRealTimers()
	})

	it('applies instance create, update, delete, status, and install-status events', async () => {
		const fixture = createFixture()
		manager = new CoreInstanceStateManager(fixture.adapter)
		await manager.start()

		const created = instanceSummary({ id: 'instance-a', path: 'Readable Server' })
		fixture.instances = [created]
		fixture.streams[0].send({ type: 'instance_created', instance: created })
		await flushAsync()
		expect(manager.snapshot.instances).toEqual([created])

		const updated = { ...created, name: 'Renamed Server' }
		fixture.instances = [updated]
		fixture.streams[0].send({ type: 'instance_updated', instance: updated })
		await flushAsync()
		expect(manager.snapshot.instances[0].name).toBe('Renamed Server')

		fixture.instances = [{ ...updated, status: 'running' }]
		fixture.streams[0].send({
			type: 'status_changed',
			instance_id: created.id,
			status: 'running',
		})
		await flushAsync()
		expect(manager.snapshot.instances[0].status).toBe('running')

		fixture.instances = [{ ...updated, status: 'running', install_status: 'failed' }]
		fixture.streams[0].send({
			type: 'install_status_changed',
			instance_id: created.id,
			install_status: 'failed',
		})
		await flushAsync()
		expect(manager.snapshot.instances[0].install_status).toBe('failed')

		fixture.instances = []
		fixture.streams[0].send({ type: 'instance_deleted', instance_id: created.id })
		await flushAsync()
		expect(manager.snapshot.instances).toEqual([])
	})

	it('refreshes one full snapshot after an event stream reconnect', async () => {
		vi.useFakeTimers()
		const beforeReconnect = instanceSummary({ id: 'instance-a', path: 'Before Reconnect' })
		const afterReconnect = instanceSummary({ id: 'instance-b', path: 'After Reconnect' })
		const fixture = createFixture([beforeReconnect])
		manager = new CoreInstanceStateManager(fixture.adapter)
		await manager.start()

		expect(manager.snapshot.instances).toEqual([beforeReconnect])
		fixture.instances = [afterReconnect]
		fixture.streams[0].close()
		await flushAsync()
		await vi.advanceTimersByTimeAsync(3000)
		await flushAsync()

		expect(fixture.streams).toHaveLength(2)
		expect(manager.snapshot.instances).toEqual([afterReconnect])
	})
})

function createFixture(initialInstances: CoreInstanceSummary[] = []) {
	let instances = initialInstances
	const streams: SseFixture[] = []
	const fetchFn = vi.fn(async (input: RequestInfo | URL, init?: RequestInit) => {
		const url = String(input)
		if (url.endsWith('/connection/handshake')) {
			const body = JSON.parse(String(init?.body))
			return Response.json({
				nonce: body.nonce,
				ok: true,
				core_id: 'core-test',
				protocol: 1,
				version: '0.1.0',
				reason: null,
			})
		}
		if (url.endsWith('/instances')) {
			return Response.json({ instances })
		}
		if (url.endsWith('/events')) {
			const stream = createSseFixture()
			streams.push(stream)
			return stream.response
		}
		return Response.json({ error: `Unhandled URL ${url}` }, { status: 404 })
	}) as unknown as typeof fetch

	const adapter: PlatformAdapter = {
		fetchFn,
		convexUrl: 'https://test.convex.cloud',
		getCoreUrl: async () => 'https://core.example.com',
		getCurrentJwt: async () => null,
		openExternalAuth: vi.fn(),
	}

	return {
		adapter,
		streams,
		get instances() {
			return instances
		},
		set instances(next: CoreInstanceSummary[]) {
			instances = next
		},
	}
}

type SseFixture = ReturnType<typeof createSseFixture>

function createSseFixture() {
	let controller: ReadableStreamDefaultController<Uint8Array> | null = null
	const encoder = new TextEncoder()
	const response = new Response(
		new ReadableStream<Uint8Array>({
			start(nextController) {
				controller = nextController
			},
		}),
		{ headers: { 'Content-Type': 'text/event-stream' } },
	)

	return {
		response,
		send(event: CoreInstanceEvent) {
			controller?.enqueue(encoder.encode(`data: ${JSON.stringify(event)}\n\n`))
		},
		close() {
			controller?.close()
		},
	}
}

function instanceSummary(overrides: Partial<CoreInstanceSummary> = {}): CoreInstanceSummary {
	return {
		id: 'instance-a',
		path: 'Readable Server',
		name: 'Readable Server',
		game_version: '1.21.4',
		loader: 'vanilla',
		loader_version: null,
		port: 25565,
		memory: { min_mb: 512, max_mb: 2048 },
		install_status: 'ready',
		status: 'offline',
		installation_id: null,
		created_at: '2026-01-01T00:00:00Z',
		updated_at: '2026-01-01T00:00:00Z',
		...overrides,
	}
}

async function flushAsync(): Promise<void> {
	await Promise.resolve()
	await Promise.resolve()
}
