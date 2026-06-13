/**
 * useAmberiteAuth — reactive auth façade that replaces the Modrinth credentials ref.
 *
 * In dev mode the "sign-in" flow seeds the dev users and stores the chosen
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
	error: ReturnType<typeof ref<Error | null>>
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

let devBootstrapStarted = false

async function ensureDevAmberiteSession(social: ReturnType<typeof useSocial>) {
	if (!import.meta.env.DEV || devBootstrapStarted) return
	devBootstrapStarted = true
	try {
		const client = useSocialClientRaw()
		const users = await client.rawMutation<
			Array<{
				userId: string
				username?: string
			}>
		>('dev:seedDevUsers', {})
		const existing = window.localStorage.getItem(DEV_ACTING_USER_KEY)
		if (!existing) {
			const pick = users.find((u) => u.username === 'amber') ?? users[0]
			if (pick) window.localStorage.setItem(DEV_ACTING_USER_KEY, pick.userId)
		}
		await social.refresh()
	} catch (e) {
		console.warn('[amberite] dev auth bootstrap failed', e)
	}
}

export function useAmberiteAuth(): UseAmberiteAuthReturn {
	const social = useSocial()
	void ensureDevAmberiteSession(social)

	const user = computed<AmberiteAuthUser | null>(() => mapToAuthUser(social.currentUser.value))
	const isLoggedIn = computed(() => !!social.currentUser.value)
	const isReady = computed(() => true)
	const signingIn = ref(false)
	const error = ref<Error | null>(null)

	async function signIn() {
		signingIn.value = true
		error.value = null
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
		} catch (e) {
			console.warn('[amberite] account connection failed', e)
			error.value = new Error(
				'Amberite could not connect through Convex. Check VITE_CONVEX_URL and make sure the Convex deployment has AMBERITE_DEV_MODE=true.',
			)
		} finally {
			signingIn.value = false
		}
	}

	async function logOut() {
		window.localStorage.removeItem(DEV_ACTING_USER_KEY)
		error.value = null
		await social.refresh()
	}

	return { user, isLoggedIn, isReady, signingIn, error, signIn, logOut }
}
