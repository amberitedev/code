import type { MaybeRefOrGetter } from 'vue'
import { computed, nextTick, onBeforeUnmount, onMounted, ref, toValue, watch } from 'vue'

export type UiLazyMountMode = 'visible' | 'idle' | 'delay' | 'immediate'

export interface UseLazyMountOptions {
	target?: MaybeRefOrGetter<Element | null | undefined>
	mode?: MaybeRefOrGetter<UiLazyMountMode | undefined>
	once?: MaybeRefOrGetter<boolean | undefined>
	contentKey?: MaybeRefOrGetter<unknown>
	delayMs?: MaybeRefOrGetter<number | undefined>
	idleTimeoutMs?: MaybeRefOrGetter<number | undefined>
	rootMargin?: MaybeRefOrGetter<string | undefined>
}

const DEFAULT_MODE: UiLazyMountMode = 'visible'
const DEFAULT_IDLE_TIMEOUT_MS = 1000
const DEFAULT_ROOT_MARGIN = '200px'

function getNumber(value: number | undefined, fallback: number) {
	if (value === undefined || !Number.isFinite(value)) return fallback

	return Math.max(0, value)
}

export function useLazyMount(options: UseLazyMountOptions = {}) {
	const mounted = ref(false)
	const shouldMount = ref(toValue(options.mode) === 'immediate')
	const hasMounted = ref(shouldMount.value)
	const signature = computed(() => ({
		contentKey: toValue(options.contentKey),
		delayMs: getNumber(toValue(options.delayMs), 0),
		idleTimeoutMs: getNumber(toValue(options.idleTimeoutMs), DEFAULT_IDLE_TIMEOUT_MS),
		mode: toValue(options.mode) ?? DEFAULT_MODE,
		once: toValue(options.once) ?? true,
		rootMargin: toValue(options.rootMargin) ?? DEFAULT_ROOT_MARGIN,
	}))

	let delayTimer: ReturnType<typeof setTimeout> | undefined
	let idleHandle: number | undefined
	let observer: IntersectionObserver | undefined
	let cleanupVersion = 0

	function clearDelayTimer() {
		if (!delayTimer) return

		clearTimeout(delayTimer)
		delayTimer = undefined
	}

	function clearIdleCallback() {
		if (idleHandle === undefined || typeof window === 'undefined') return

		const idleWindow = window as Window & {
			cancelIdleCallback?: (handle: number) => void
		}
		idleWindow.cancelIdleCallback?.(idleHandle)
		idleHandle = undefined
	}

	function clearObserver() {
		observer?.disconnect()
		observer = undefined
	}

	function cleanup() {
		cleanupVersion++
		clearDelayTimer()
		clearIdleCallback()
		clearObserver()
	}

	function markMounted() {
		shouldMount.value = true
		hasMounted.value = true

		if (signature.value.once) {
			cleanup()
		}
	}

	function startVisibleMode(version: number) {
		if (typeof window === 'undefined') return

		const startObserver = () => {
			if (!mounted.value || version !== cleanupVersion) return

			const target = toValue(options.target)
			if (!target) {
				markMounted()
				return
			}

			if (typeof IntersectionObserver === 'undefined') {
				markMounted()
				return
			}

			observer = new IntersectionObserver(
				(entries) => {
					const visible = entries.some((entry) => entry.isIntersecting)
					if (visible) {
						markMounted()
					} else if (!signature.value.once) {
						shouldMount.value = false
					}
				},
				{ rootMargin: signature.value.rootMargin },
			)
			observer.observe(target)
		}

		void nextTick(startObserver)
	}

	function startIdleMode() {
		if (typeof window === 'undefined') return

		const idleWindow = window as Window & {
			requestIdleCallback?: (callback: () => void, options?: { timeout?: number }) => number
		}

		if (idleWindow.requestIdleCallback) {
			idleHandle = idleWindow.requestIdleCallback(markMounted, {
				timeout: signature.value.idleTimeoutMs,
			})
			return
		}

		delayTimer = setTimeout(markMounted, signature.value.idleTimeoutMs)
	}

	function startDelayMode() {
		if (typeof window === 'undefined') return

		delayTimer = setTimeout(markMounted, signature.value.delayMs)
	}

	function start() {
		cleanup()
		const version = cleanupVersion

		if (signature.value.mode === 'immediate') {
			markMounted()
			return
		}

		if (signature.value.once && hasMounted.value) {
			shouldMount.value = true
			return
		}

		shouldMount.value = false

		switch (signature.value.mode) {
			case 'idle':
				startIdleMode()
				break
			case 'delay':
				startDelayMode()
				break
			default:
				startVisibleMode(version)
				break
		}
	}

	function reset() {
		cleanup()
		hasMounted.value = signature.value.mode === 'immediate'
		shouldMount.value = signature.value.mode === 'immediate'

		if (mounted.value && !shouldMount.value) {
			start()
		}
	}

	function trigger() {
		markMounted()
	}

	onMounted(() => {
		mounted.value = true
		if (!shouldMount.value) {
			start()
		}
	})

	onBeforeUnmount(cleanup)

	watch(signature, () => {
		if (!mounted.value) {
			hasMounted.value = signature.value.mode === 'immediate'
			shouldMount.value = signature.value.mode === 'immediate'
			return
		}

		reset()
	})

	return {
		hasMounted,
		reset,
		shouldMount,
		trigger,
	}
}
