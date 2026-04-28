import { ref } from 'vue'

const OPENCODE_URL = 'http://localhost:4096'

export interface CommentPayload {
	id: string
	component: string
	source_file: string
	source_lines?: [number, number]
	html: string
	css_classes: string[]
	parent?: { component: string; file: string }
	comment: string
	timestamp: string
}

/**
 * Provides submit(), loading, and error for POSTing a design comment
 * to the opencode server at localhost:4096/design-comments.
 */
export function useCommentSubmit() {
	const loading = ref(false)
	const error = ref<string | null>(null)

	async function submit(payload: CommentPayload): Promise<boolean> {
		loading.value = true
		error.value = null
		try {
			const res = await fetch(`${OPENCODE_URL}/design-comments`, {
				method: 'POST',
				headers: { 'Content-Type': 'application/json' },
				body: JSON.stringify(payload),
			})
			if (!res.ok) throw new Error(`Server responded ${res.status}`)
			return true
		} catch (e) {
			error.value = e instanceof Error ? e.message : 'Failed to send comment'
			return false
		} finally {
			loading.value = false
		}
	}

	return { submit, loading, error }
}
