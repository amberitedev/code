/**
 * useAmberiteAuth — reactive auth façade that replaces the Modrinth credentials ref.
 *
 * The sign-in flow runs Microsoft OAuth through the Tauri shell, stores the
 * returned Amberite session JWT, and refreshes the Convex-backed social profile.
 *
 * The mapped user object exposes the Labrinth.Users.v2.User shape so
 * @modrinth/ui components and existing auth-gated pages keep working without
 * churn.
 */
import { computed, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { useSocialClientRaw } from '@/composables/useSocialClient'
import { login as amberiteLogin } from '@/helpers/amberite_auth'

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

export function useAmberiteAuth(): UseAmberiteAuthReturn {
	const social = useSocial()
	const adapter = useCoreClient().adapter

	const user = computed<AmberiteAuthUser | null>(() => mapToAuthUser(social.currentUser.value))
	const isLoggedIn = computed(() => !!social.currentUser.value)
	const isReady = computed(() => !social.loading.value)
	const signingIn = ref(false)
	const error = ref<Error | null>(null)

	async function signIn() {
		signingIn.value = true
		error.value = null
		try {
			const credential = await amberiteLogin()
			const session = await useSocialClientRaw().rawAction<{
				tokens: { token: string; refreshToken: string } | null
			}>('auth:signIn', {
				provider: 'minecraft-token',
				params: { minecraftAccessToken: credential.accessToken },
			})
			if (!session.tokens) throw new Error('Amberite account session was not accepted.')
			await adapter.setCurrentJwt?.(session.tokens.token)
			await social.refresh()
			if (!social.currentUser.value) throw new Error('Amberite account session was not accepted.')
		} catch (e) {
			console.warn('[amberite] account connection failed', e)
			error.value = new Error(
				'Amberite could not connect your account. Check your connection and try again.',
			)
		} finally {
			signingIn.value = false
		}
	}

	async function logOut() {
		await adapter.setCurrentJwt?.(null)
		error.value = null
		await social.refresh()
	}

	return { user, isLoggedIn, isReady, signingIn, error, signIn, logOut }
}
