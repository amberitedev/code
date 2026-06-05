import { computed, ref } from 'vue'

type CorePreviewState = 'setup' | 'local' | 'external'
type FakeUser = {
	userId: string
	username: string
	displayName: string
	image: null
	friendCode: string
}

const storageKey = 'amberite-core-preview-state'
const loginStorageKey = 'amberite-core-preview-logged-in'
const previewState = ref<CorePreviewState>(
	typeof localStorage === 'undefined'
		? 'setup'
		: ((localStorage.getItem(storageKey) as CorePreviewState | null) ?? 'setup'),
)
const previewLoggedIn = ref(
	typeof localStorage === 'undefined' ? true : localStorage.getItem(loginStorageKey) !== 'false',
)

const fakeUsers: FakeUser[] = [
	['ilai', 'Ilai'],
	['maya', 'Maya'],
	['noam', 'Noam'],
	['lina', 'Lina'],
	['ori', 'Ori'],
	['tamar', 'Tamar'],
	['eden', 'Eden'],
	['shai', 'Shai'],
	['ron', 'Ron'],
	['yael', 'Yael'],
	['aviv', 'Aviv'],
	['dana', 'Dana'],
	['guy', 'Guy'],
	['rani', 'Rani'],
	['omer', 'Omer'],
	['neta', 'Neta'],
	['lior', 'Lior'],
	['amir', 'Amir'],
	['gal', 'Gal'],
	['ziv', 'Ziv'],
].map(([username, displayName], index) => ({
	userId: `preview-user-${index + 1}`,
	username,
	displayName,
	image: null,
	friendCode: `AMB-${String(index + 1).padStart(4, '0')}`,
}))

const fakeFriends = fakeUsers.slice(1, 4).map((user, index) => ({
	friendshipId: `preview-friend-${index + 1}`,
	status: 'accepted' as const,
	user,
}))

export function useCorePreview() {
	const setPreviewState = (state: CorePreviewState) => {
		previewState.value = state
		localStorage.setItem(storageKey, state)
	}
	const setPreviewLoggedIn = (value: boolean) => {
		previewLoggedIn.value = value
		localStorage.setItem(loginStorageKey, value ? 'true' : 'false')
	}

	return {
		previewState,
		previewLoggedIn,
		isPreviewConnected: computed(() => previewState.value !== 'setup'),
		isPreviewLocal: computed(() => previewState.value === 'local'),
		fakeUsers,
		fakeFriends,
		setPreviewLoggedIn,
		setPreviewState,
	}
}
