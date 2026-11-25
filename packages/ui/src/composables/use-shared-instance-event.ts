import type { SharedInstanceList } from '@amberite/contracts/backend'
import { type MaybeRefOrGetter, onScopeDispose, toValue } from 'vue'

import { injectModrinthClient } from '#ui/providers'

/**
 * Calls `onChange` when the backend says `type` of the shared instance `sharedInstanceId` changed,
 * so a page showing it fetches again instead of polling. Also called when the friends socket
 * (re)connects, since events sent while it was down are lost.
 */
export function useSharedInstanceEvent(
	type: SharedInstanceList,
	sharedInstanceId: MaybeRefOrGetter<string | null | undefined>,
	onChange: () => void,
) {
	const events = injectModrinthClient().localEvents
	const stops = [
		events.on('notification', (notification) => {
			const body =
				notification && typeof notification === 'object' && 'body' in notification
					? notification.body
					: null
			if (
				body &&
				typeof body === 'object' &&
				'type' in body &&
				'shared_instance_id' in body &&
				body.type === type &&
				body.shared_instance_id === toValue(sharedInstanceId)
			)
				onChange()
		}),
		events.on('friend', (event) => {
			if (event.event === 'status_sync') onChange()
		}),
	]
	onScopeDispose(() => {
		for (const stop of stops) stop()
	})
}
