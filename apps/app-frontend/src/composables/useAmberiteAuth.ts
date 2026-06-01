/**
 * useAmberiteAuth — reactive auth façade that replaces the Modrinth credentials ref.
 *
 * In dev mode the "sign-in" flow seeds the 4 dev users and stores the chosen
 * Convex user id in localStorage so every Convex call carries __actAs. In
 * production this will be replaced by a Microsoft-OAuth → Amberite account
 * linkage (auto-create on first login).
 *
 * The mapped user object exposes the Labrinth.Users.v2.User shape so
 * @modrinth/ui components and existing auth-gated pages keep working without
 * churn.
 */
import { computed, ref } from 'vue'

import { DEV_ACTING_USER_KEY } from '@/adapters/desktop'
import { useSocial } from '@/composables/useSocial'
import { useSocialClientRaw } from '@/composables/useSocialClient'

export interface AmberiteAuthUser {
	id: string
	username: string
	name: string
	avatar_url: string
	role: string
	badges: number
	created: string
	bio?: string
	email?: string | null
}

export interface UseAmberiteAuthReturn {
	user: ReturnType<typeof computed<AmberiteAuthUser | null>>
	isLoggedIn: ReturnType<typeof computed<boolean>>
	isReady: ReturnType<typeof computed<boolean>>
	signingIn: ReturnType<typeof ref<boolean>>
	signIn: () => Promise<void>
	logOut: () => Promise<void>
}

function mapToAuthUser(
	cu: {
		userId: string
		username?: string
		displayName?: string
		image?: string
		createdAt?: number
	} | null,
): AmberiteAuthUser | null {
	if (!cu) return null
	const username = cu.username ?? cu.displayName ?? 'User'
	return {
		id: cu.userId,
		username,
		name: cu.displayName ?? username,
		avatar_url: cu.image ?? '',
		role: '',
		badges: 0,
		created: cu.createdAt ? new Date(cu.createdAt).toISOString() : new Date().toISOString(),
		bio: '',
		email: null,
	}
}

export function useAmberiteAuth(): UseAmberiteAuthReturn {
	const social = useSocial()

	const user = computed<AmberiteAuthUser | null>(() => mapToAuthUser(social.currentUser.value))
	const isLoggedIn = computed(() => !!social.currentUser.value)
	const isReady = computed(() => true)
	const signingIn = ref(false)

	async function signIn() {
		signingIn.value = true
		try {
			const client = useSocialClientRaw()
			await client.rawMutation('dev:seedDevUsers', {})
			const users = await client.rawQuery<AmberiteAuthUser[]>('dev:listDevUsers', {})
			const pick = users.find((u: Record<string, unknown>) => u.username === 'amber') ?? users[0]
			if (pick) {
				window.localStorage.setItem(
					DEV_ACTING_USER_KEY,
					String((pick as Record<string, unknown>).userId),
				)
				await social.refresh()
			}
		} finally {
			signingIn.value = false
		}
	}

	async function logOut() {
		window.localStorage.removeItem(DEV_ACTING_USER_KEY)
		await social.refresh()
	}

	return { user, isLoggedIn, isReady, signingIn, signIn, logOut }
}
