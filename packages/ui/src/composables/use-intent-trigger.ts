import type { MaybeRefOrGetter } from 'vue'
import { computed, onBeforeUnmount, toValue, watch } from 'vue'

export type IntentTriggerSource = 'hover' | 'focus' | 'pointerdown' | 'manual'
export type IntentTriggerEventSource = Exclude<IntentTriggerSource, 'manual'>

export interface IntentTriggerContext {
	source: IntentTriggerSource
	event?: Event
}

export interface UseIntentTriggerOptions {
	triggers?: MaybeRefOrGetter<readonly IntentTriggerEventSource[] | undefined>
	once?: MaybeRefOrGetter<boolean | undefined>
	disabled?: MaybeRefOrGetter<boolean | undefined>
	delayMs?: MaybeRefOrGetter<number | undefined>
	onError?: (error: unknown) => void
}

const DEFAULT_TRIGGERS: IntentTriggerEventSource[] = ['hover', 'focus', 'pointerdown']

function getDelayMs(value: number | undefined) {
	if (value === undefined || !Number.isFinite(value)) return 0

	return Math.max(0, value)
}

export function useIntentTrigger(
	callback: (context: IntentTriggerContext) => unknown | Promise<unknown>,
	options: UseIntentTriggerOptions = {},
) {
	let hasTriggered = false
	let running = false
	let timer: ReturnType<typeof setTimeout> | undefined

	function clearTimer() {
		if (!timer) return

		clearTimeout(timer)
		timer = undefined
	}

	function getTriggers() {
		return new Set(toValue(options.triggers) ?? DEFAULT_TRIGGERS)
	}

	function isDisabled() {
		return toValue(options.disabled) ?? false
	}

	function isOnce() {
		return toValue(options.once) ?? true
	}

	function handleError(error: unknown) {
		try {
			options.onError?.(error)
		} catch {
			// DOM intent handlers should not surface callback or error-handler failures.
		}
	}

	async function run(context: IntentTriggerContext) {
		running = true
		try {
			await callback(context)
		} catch (error) {
			handleError(error)
		} finally {
			running = false
		}
	}

	function trigger(source: IntentTriggerSource = 'manual', event?: Event) {
		if (isDisabled()) return false
		if (timer || running) return false
		if (isOnce() && hasTriggered) return false

		hasTriggered = true
		const context = { event, source }
		const delayMs = getDelayMs(toValue(options.delayMs))

		if (delayMs > 0) {
			timer = setTimeout(() => {
				timer = undefined
				void run(context)
			}, delayMs)
			return true
		}

		void run(context)
		return true
	}

	function reset() {
		clearTimer()
		hasTriggered = false
		running = false
	}

	const intentProps = computed(() => {
		const triggers = getTriggers()

		return {
			onFocus: triggers.has('focus')
				? (event: FocusEvent) => {
						trigger('focus', event)
					}
				: undefined,
			onMouseenter: triggers.has('hover')
				? (event: MouseEvent) => {
						trigger('hover', event)
					}
				: undefined,
			onPointerdown: triggers.has('pointerdown')
				? (event: PointerEvent) => {
						trigger('pointerdown', event)
					}
				: undefined,
		}
	})

	watch(
		() => toValue(options.disabled),
		(disabled) => {
			if (disabled) {
				clearTimer()
			}
		},
	)

	onBeforeUnmount(clearTimer)

	return {
		intentProps,
		reset,
		trigger,
	}
}
