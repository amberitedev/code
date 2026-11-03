import { computed, type MaybeRefOrGetter, shallowRef, toValue, watch } from 'vue'

import { injectIconCache } from '#ui/providers/icon-cache'

<<<<<<< HEAD
export function useCachedIcon(source: MaybeRefOrGetter<string | Blob | null | undefined>) {
	const cache = injectIconCache(null)
	const input = computed(() => toValue(source) ?? undefined)
	const resolved = shallowRef<{ source: string | Blob; url: string }>()
=======
export function useCachedIcon(source: MaybeRefOrGetter<string | null | undefined>) {
	const cache = injectIconCache(null)
	const input = computed(() => toValue(source) ?? undefined)
	if (!cache) return { src: input, pending: computed(() => false) }

	const isCacheable = (value: string | undefined): value is string =>
		!!value?.startsWith('https://')
	const resolved = shallowRef<{ source: string; url: string }>()
>>>>>>> modrinth/main

	watch(
		input,
		async (value, _, onCleanup) => {
<<<<<<< HEAD
			if (!value || typeof window === 'undefined') {
				resolved.value = undefined
				return
			}
			if (typeof value === 'string' && (!cache || !value.startsWith('https://'))) return
			const cached = cache?.peekIcon?.(value)
=======
			if (!isCacheable(value)) return
			const cached = cache.peekIcon(value)
>>>>>>> modrinth/main
			if (cached) {
				resolved.value = { source: value, url: cached }
				return
			}
			let active = true
<<<<<<< HEAD
			let objectUrl: string | undefined
			onCleanup(() => {
				active = false
				if (!objectUrl) return
				if (resolved.value?.url === objectUrl) resolved.value = undefined
				URL.revokeObjectURL(objectUrl)
			})
			const fallback = () => {
				if (typeof value === 'string') return value
				objectUrl = URL.createObjectURL(value)
				return objectUrl
			}
			try {
				const url = cache ? await cache.cacheIcon(value) : fallback()
				if (active) resolved.value = { source: value, url }
			} catch (error) {
				console.warn('Could not cache icon', error)
				if (active) resolved.value = { source: value, url: fallback() }
=======
			onCleanup(() => {
				active = false
			})
			try {
				const url = await cache.cacheIcon(value)
				if (active) resolved.value = { source: value, url }
			} catch (error) {
				console.warn('Could not cache icon', error)
				if (active) resolved.value = { source: value, url: value }
>>>>>>> modrinth/main
			}
		},
		{ immediate: true },
	)

<<<<<<< HEAD
	return computed(() => {
		const value = input.value
		if (!value) return undefined
		if (typeof value === 'string' && (!cache || !value.startsWith('https://'))) return value
		return resolved.value?.url
	})
=======
	const pending = computed(() => isCacheable(input.value) && resolved.value?.source !== input.value)
	const src = computed(() => {
		const value = input.value
		if (!isCacheable(value)) return value
		return pending.value ? undefined : resolved.value?.url
	})

	return { src, pending }
>>>>>>> modrinth/main
}
