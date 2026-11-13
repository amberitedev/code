import { ContractClient, events as eventRoutes, type LocalEvent } from '@modrinth/api-client'
import { useQueryClient } from '@tanstack/vue-query'
import { onScopeDispose } from 'vue'

import { localTransport } from '@/platform/local'
import { dispatchOpEvent } from '@/platform/ops'
import {
	type AppEventHandler,
	type AppEvents,
	type AppEventType,
	provideAppEvents,
} from '@/providers/app-events'

type UntypedAppEventHandler = (payload: unknown) => void | Promise<void>

const REOPEN_AFTER_MS = 1000

export function setupAppEventsProvider() {
	const queryClient = useQueryClient()
	const handlers = new Map<AppEventType, Set<UntypedAppEventHandler>>()

	function on<Type extends AppEventType>(type: Type, handler: AppEventHandler<Type>) {
		let eventHandlers = handlers.get(type)
		if (!eventHandlers) {
			eventHandlers = new Set()
			handlers.set(type, eventHandlers)
		}

		const untypedHandler = handler as unknown as UntypedAppEventHandler
		eventHandlers.add(untypedHandler)
		return () => {
			eventHandlers.delete(untypedHandler)
			if (eventHandlers.size === 0) handlers.delete(type)
		}
	}

	function once<Type extends AppEventType>(type: Type, handler: AppEventHandler<Type>) {
		let unsubscribe = () => {}
		unsubscribe = on(type, async (payload) => {
			unsubscribe()
			await handler(payload)
		})
		return unsubscribe
	}

	const events: AppEvents = { on, once }

	function dispatch(event: LocalEvent) {
		// The socket fell behind and lost events, so nothing shown can be trusted.
		if (event.type === 'resync') void queryClient.invalidateQueries()
		dispatchOpEvent(event)
		const eventHandlers = handlers.get(event.type)
		if (!eventHandlers) return

		const payload = 'payload' in event ? event.payload : undefined
		for (const handler of [...eventHandlers]) {
			void Promise.resolve()
				.then(() => handler(payload))
				.catch((error) => {
					console.error(`Unhandled ${event.type} app event`, error)
				})
		}
	}

	const url = new ContractClient(localTransport)
		.url(eventRoutes.eventSocket, null)
		.replace(/^http/, 'ws')
	let socket: WebSocket | undefined
	let disposed = false

	/** Resolves once the socket is open: every event raised after that reaches the handlers. */
	function connect(): Promise<void> {
		return new Promise((resolve, reject) => {
			const opening = new WebSocket(url)
			opening.onmessage = (message) => dispatch(JSON.parse(message.data) as LocalEvent)
			opening.onopen = () => {
				socket = opening
				resolve()
			}
			opening.onerror = () => reject(new Error("The App's event socket failed"))
			opening.onclose = () => {
				if (socket === opening && !disposed) reopen()
			}
		})
	}

	// Events raised while the socket was closed are lost, so a reopened socket is a resync.
	function reopen() {
		setTimeout(() => {
			if (disposed) return
			void connect().then(() => dispatch({ type: 'resync' }), reopen)
		}, REOPEN_AFTER_MS)
	}

	provideAppEvents(events)

	function dispose() {
		disposed = true
		socket?.close()
		handlers.clear()
	}

	onScopeDispose(dispose)

	return { connect, events, dispose }
}
