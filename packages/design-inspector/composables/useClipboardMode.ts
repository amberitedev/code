import { onMounted, onUnmounted, ref } from 'vue'

const STORAGE_KEY = 'di:autoClipboard'

const diLog = import.meta.env.DEV
	? (...args: unknown[]) => console.log('[DesignInspector][ClipboardMode]', ...args)
	: () => {}

/** Singleton state — shared across all composable calls in the same page. */
const autoClipboard = ref(
	typeof localStorage !== 'undefined' && localStorage.getItem(STORAGE_KEY) === 'true',
)
const infoMessage = ref<string | null>(null)
let dismissTimer: ReturnType<typeof setTimeout> | null = null

function showInfo(msg: string) {
	if (dismissTimer) clearTimeout(dismissTimer)
	infoMessage.value = msg
	dismissTimer = setTimeout(() => {
		infoMessage.value = null
	}, 2000)
}

/**
 * Copy a comment payload to the clipboard if auto-clipboard mode is enabled.
 * Call this before or after submit — it's fire-and-forget.
 */
export async function copyPayloadToClipboard(payload: object): Promise<void> {
	if (!autoClipboard.value) return
	try {
		await navigator.clipboard.writeText(JSON.stringify(payload, null, 2))
		diLog('Payload copied to clipboard:', payload)
	} catch (e) {
		diLog('Clipboard write failed:', e)
	}
}

function onKeyDown(e: KeyboardEvent) {
	// Alt+Shift+C — toggle auto-clipboard mode
	if (e.altKey && e.shiftKey && (e.key === 'c' || e.key === 'C')) {
		e.preventDefault()
		autoClipboard.value = !autoClipboard.value
		localStorage.setItem(STORAGE_KEY, String(autoClipboard.value))
		showInfo(autoClipboard.value ? 'Auto-clipboard ON' : 'Auto-clipboard OFF')
		diLog('Auto-clipboard toggled:', autoClipboard.value)
	}
}

/**
 * Mount in DesignInspectorRoot to register the Alt+Shift+C toggle keybind.
 * Returns reactive state for rendering the status indicator.
 */
export function useClipboardMode() {
	onMounted(() => window.addEventListener('keydown', onKeyDown, { capture: true }))
	onUnmounted(() => window.removeEventListener('keydown', onKeyDown, { capture: true }))
	return { autoClipboard, infoMessage }
}
