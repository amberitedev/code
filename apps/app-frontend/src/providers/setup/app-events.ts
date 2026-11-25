import { useQueryClient } from '@tanstack/vue-query'
import { onScopeDispose } from 'vue'

import { apiClient } from '@/platform/modrinth-client'
import { dispatchOpEvent } from '@/platform/ops'
import {
	type AppEventHandler,
	type AppEvents,
	type AppEventType,
	provideAppEvents,
} from '@/providers/app-events'

/**
 * The App's events, from its one event socket (`apiClient.localEvents`). A resync, which the
 * socket hands over when it reopened or fell behind, refetches every query; its first open lost
 * nothing, so the App's start refetches nothing.
 */
export function setupAppEventsProvider() {
	const queryClient = useQueryClient()
	const stream = apiClient.localEvents

	function on<Type extends AppEventType>(type: Type, handler: AppEventHandler<Type>) {
		return stream.on(type, (payload) => {
			void Promise.resolve()
				.then(() => handler(payload as Parameters<AppEventHandler<Type>>[0]))
				.catch((error) => {
					console.error(`Unhandled ${type} app event`, error)
				})
		})
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
	const stops = [
		stream.on('resync', () => void queryClient.invalidateQueries()),
		stream.onAny(dispatchOpEvent),
	]

	provideAppEvents(events)

	function dispose() {
		for (const stop of stops) stop()
	}

	onScopeDispose(dispose)

	/** Resolves once the socket is open: every event raised after that reaches the handlers. */
	const connect = () => stream.ready()

	return { connect, events, dispose }
}
