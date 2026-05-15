import { ref } from 'vue'

// All requests go through the Vite dev server relay at /__design-relay.
// The relay proxies to the T3 Code dev server on the fixed port (3773).
// No lock file or URL discovery — T3 Code always runs on port 3773.
const RELAY = '/__design-relay'

export interface CommentPayload {
	id: string
	component: string
	'source file': string
	comment: string
}

const diLog = import.meta.env.DEV
	? (...args: unknown[]) => console.log('[DesignInspector][Amberite]', ...args)
	: () => {}

/**
 * Provides submit(), loading, and error for POSTing a design comment
 * to the opencode server via the Vite relay proxy.
 */
export function useCommentSubmit() {
	const loading = ref(false)
	const error = ref<string | null>(null)

	async function submit(payload: CommentPayload): Promise<boolean> {
		loading.value = true
		error.value = null
		diLog('Submitting comment payload:', JSON.stringify(payload, null, 2))
		try {
			const res = await fetch(`${RELAY}/design-comments`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(payload),
			})
			diLog('Server response status:', res.status, res.statusText)
			if (!res.ok) {
				const body = await res.text().catch(() => '(no body)')
				diLog('Server response FAILED body:', body)
				throw new Error(`Server responded ${res.status}: ${body}`)
			}
			const json = await res.json().catch(() => null)
			diLog('Server response OK, body:', json)
			return true
		} catch (e) {
			diLog('submit() CAUGHT ERROR:', e)
			error.value = e instanceof Error ? e.message : 'Failed to send comment'
			return false
		} finally {
			loading.value = false
		}
	}

	return { submit, loading, error }
}
