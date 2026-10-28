import type { SharedInstances } from '@modrinth/api-client'
import { type MaybeRefOrGetter, toValue } from 'vue'

import { useAppEvent } from '@/composables/use-app-event'

/**
 * Calls `onChange` when the backend says `type` of the shared instance `sharedInstanceId` changed,
 * so a page showing it fetches again instead of polling. Also called when the friends socket
 * (re)connects, since events sent while it was down are lost.
 */
export function useSharedInstanceEvent(
	type: SharedInstances.LiveEvent['body']['type'],
	sharedInstanceId: MaybeRefOrGetter<string | null | undefined>,
	onChange: () => void,
) {
	useAppEvent('notification', (notification) => {
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
	})
	useAppEvent('friend', (event) => {
		if (event.event === 'status_sync') onChange()
	})
}
