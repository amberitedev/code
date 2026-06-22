import { describe, expect, it } from 'vitest'

import { RealtimePresenceSession, type RealtimeSocket } from '../realtime'

class FakeSocket implements RealtimeSocket {
	readonly CONNECTING = 0
	readonly OPEN = 1
	readyState = this.CONNECTING
	onopen: (() => void) | null = null
	onmessage: ((event: { data: unknown }) => void) | null = null
	onclose: (() => void) | null = null
	onerror: (() => void) | null = null
	closeCode: number | undefined

	open(): void {
		this.readyState = this.OPEN
		this.onopen?.()
	}

	receive(data: unknown): void {
		this.onmessage?.({ data })
	}

	close(code?: number): void {
		this.closeCode = code
		this.readyState = 3
		this.onclose?.()
	}
}

describe('RealtimePresenceSession', () => {
	it('validates presence frames and closes on invalidation', async () => {
		const frames: unknown[] = []
		let invalidated = false
		let socket: FakeSocket | null = null
		let url = ''
		const session = new RealtimePresenceSession({
			endpoint: 'https://realtime.example',
			fetchFn: async () => Response.json({ ticket: 'a'.repeat(32) }),
			createWebSocket: (nextUrl) => {
				url = nextUrl
				socket = new FakeSocket()
				return socket
			},
			getJwt: async () => 'session-token',
			onFrame: (frame) => frames.push(frame),
			onInvalidated: () => {
				invalidated = true
			},
		})

		await session.connect()
		expect(url).toBe('wss://realtime.example/v1/connect?ticket=' + 'a'.repeat(32))
		socket?.open()
		socket?.receive(JSON.stringify({ type: 'presence.user', userId: 'user-1', online: true }))
		socket?.receive(JSON.stringify({ type: 'presence.user', userId: 1, online: true }))
		expect(frames).toEqual([{ type: 'presence.user', userId: 'user-1', online: true }])

		socket?.receive(JSON.stringify({ type: 'authorization.invalidated' }))
		expect(invalidated).toBe(true)
		expect(socket?.closeCode).toBe(1000)
	})
})
