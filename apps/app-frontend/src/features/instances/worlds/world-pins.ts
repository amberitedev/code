import { useStorage } from '@vueuse/core'
import { computed, type MaybeRefOrGetter, toValue } from 'vue'

import type { World } from '@/helpers/worlds'

type InstancePins = {
	/** Row keys in the order the user dragged them. */
	pinned: string[]
	/** When a row that is not in the instance's own server list was last joined from this App. */
	joined: Record<string, string>
}

/** The key a world or server from the instance's own list is pinned under. */
export function worldPinKey(world: World) {
	return world.type === 'singleplayer' ? `world:${world.path}` : `server:${world.address}`
}

/**
 * What the Worlds tab remembers about one instance across restarts: which rows are pinned to the
 * top and in what order. Rows are named by key, so a key whose row is gone is simply never shown.
 */
export function useWorldPins(instanceId: MaybeRefOrGetter<string>) {
	const stored = useStorage<Record<string, InstancePins>>('instance-worlds-pins', {}, localStorage)
	const current = computed(() => stored.value[toValue(instanceId)])

	function update(change: (pins: InstancePins) => InstancePins) {
		stored.value = {
			...stored.value,
			[toValue(instanceId)]: change(current.value ?? { pinned: [], joined: {} }),
		}
	}

	const pinned = computed({
		get: () => current.value?.pinned ?? [],
		set: (keys) => update((pins) => ({ ...pins, pinned: keys })),
	})

	return {
		pinned,
		/** Pins a row at the end of the pinned rows, or unpins it. */
		toggle(key: string) {
			pinned.value = pinned.value.includes(key)
				? pinned.value.filter((pinnedKey) => pinnedKey !== key)
				: [...pinned.value, key]
		},
		lastJoined: (key: string) => current.value?.joined[key],
		markJoined(key: string) {
			update((pins) => ({ ...pins, joined: { ...pins.joined, [key]: new Date().toISOString() } }))
		},
	}
}
