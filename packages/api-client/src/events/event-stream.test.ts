import { afterEach, beforeEach, describe, expect, it, vi } from 'vite-plus/test'

import { EventStream } from './event-stream'

class FakeSocket {
	static OPEN = 1
	static opened: FakeSocket[] = []
	readyState = 0
	sent: unknown[] = []
	onopen = () => {}
	onmessage: (message: { data: string }) => void = () => {}
	onclose = () => {}

	constructor(readonly url: string) {
		FakeSocket.opened.push(this)
	}

	send(text: string) {
		this.sent.push(JSON.parse(text))
	}

	close() {}

	open() {
		this.readyState = FakeSocket.OPEN
		this.onopen()
	}

	receive(event: unknown) {
		this.onmessage({ data: JSON.stringify(event) })
	}

	drop() {
		this.readyState = 3
		this.onclose()
	}
}

type Event = { type: 'ping'; payload: number } | { type: 'resync' }

function stream() {
	return new EventStream<Event, string>({
		url: async () => 'ws://core/events',
		message: (topic, subscribe) => ({ topic, subscribe }),
	})
}

/** Lets the stream's pending `url()` resolve. */
const settle = () => vi.advanceTimersByTimeAsync(0)

describe('an event stream', () => {
	beforeEach(() => {
		vi.useFakeTimers()
		FakeSocket.opened = []
		vi.stubGlobal('WebSocket', FakeSocket)
	})
	afterEach(() => {
		vi.useRealTimers()
		vi.unstubAllGlobals()
	})

	it('shares one socket and subscribes each topic once', async () => {
		const events = stream()
		const heard: number[] = []
		events.on('ping', (payload) => heard.push(payload))
		const first = events.subscribe('server-a')
		const second = events.subscribe('server-a')
		await settle()
		const socket = FakeSocket.opened[0]!
		socket.open()
		socket.receive({ type: 'ping', payload: 1 })

		expect(FakeSocket.opened).toHaveLength(1)
		expect(socket.sent).toEqual([{ topic: 'server-a', subscribe: true }])
		expect(heard).toEqual([1])
		first()
		expect(socket.sent).toHaveLength(1)
		second()
		expect(socket.sent.at(-1)).toEqual({ topic: 'server-a', subscribe: false })
	})

	it('resyncs after it reopens, not when it first opens', async () => {
		const events = stream()
		const resyncs: unknown[] = []
		const status: boolean[] = []
		events.on('resync', () => resyncs.push('resync'))
		events.onStatus((open) => status.push(open))
		events.subscribe('server-a')
		await settle()
		FakeSocket.opened[0]!.open()
		expect(resyncs).toEqual([])

		FakeSocket.opened[0]!.drop()
		await vi.advanceTimersByTimeAsync(1000)
		const reopened = FakeSocket.opened[1]!
		reopened.open()

		expect(resyncs).toEqual(['resync'])
		expect(status).toEqual([true, false, true])
		expect(reopened.sent).toEqual([{ topic: 'server-a', subscribe: true }])
	})
})
