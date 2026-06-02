import { computed, ref } from 'vue'

type CorePreviewState = 'setup' | 'local' | 'external'

const storageKey = 'amberite-core-preview-state'
const previewState = ref<CorePreviewState>(
	typeof localStorage === 'undefined'
		? 'setup'
		: ((localStorage.getItem(storageKey) as CorePreviewState | null) ?? 'setup'),
)

export function useCorePreview() {
	const setPreviewState = (state: CorePreviewState) => {
		previewState.value = state
		localStorage.setItem(storageKey, state)
	}

	return {
		previewState,
		isPreviewConnected: computed(() => previewState.value !== 'setup'),
		isPreviewLocal: computed(() => previewState.value === 'local'),
		setPreviewState,
	}
}
