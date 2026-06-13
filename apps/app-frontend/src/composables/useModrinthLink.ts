import type { Labrinth } from '@modrinth/api-client'
import { computed, ref } from 'vue'

import { get_user } from '@/helpers/cache'
import {
	get as getModrinthCredentials,
	login as linkModrinthAccount,
	logout as unlinkModrinthAccount,
	type ModrinthCredentials,
} from '@/helpers/mr_auth'

const credentials = ref<ModrinthCredentials | null>(null)
const user = ref<Labrinth.Users.v2.User | null>(null)
const loading = ref(false)
const error = ref<Error | null>(null)
let initialized = false

export function useModrinthLink() {
	const linked = computed(() => !!credentials.value?.session && !!credentials.value?.user_id)

	async function refresh(): Promise<void> {
		loading.value = true
		error.value = null
		try {
			const nextCredentials = await getModrinthCredentials()
			credentials.value = nextCredentials ?? null
			if (nextCredentials?.user_id) {
				user.value = ((await get_user(nextCredentials.user_id, 'bypass')) ??
					null) as Labrinth.Users.v2.User | null
			} else {
				user.value = null
			}
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			credentials.value = null
			user.value = null
		} finally {
			loading.value = false
		}
	}

	async function link(): Promise<void> {
		loading.value = true
		error.value = null
		try {
			const nextCredentials = await linkModrinthAccount()
			credentials.value = nextCredentials
			user.value = nextCredentials.user_id
				? (((await get_user(nextCredentials.user_id, 'bypass')) ??
						null) as Labrinth.Users.v2.User | null)
				: null
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			throw e
		} finally {
			loading.value = false
		}
	}

	async function unlink(): Promise<void> {
		loading.value = true
		error.value = null
		try {
			await unlinkModrinthAccount()
			credentials.value = null
			user.value = null
		} catch (e) {
			error.value = e instanceof Error ? e : new Error(String(e))
			throw e
		} finally {
			loading.value = false
		}
	}

	if (!initialized) {
		initialized = true
		void refresh()
	}

	return {
		credentials,
		user,
		linked,
		loading,
		error,
		refresh,
		link,
		unlink,
	}
}
