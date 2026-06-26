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
import {
	ConvexAmberiteAuthClient,
	mapAmberiteUserToAccountUser,
	type AmberiteAccountUser,
} from '@amberite/amberite-api'
import { invoke } from '@tauri-apps/api/core'
import type { ComputedRef, Ref } from 'vue'
import { computed, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'
import { login as amberiteLogin } from '@/helpers/amberite_auth'

const MIN_SIGN_IN_VISIBLE_MS = 2500
const SIGN_IN_TIMEOUT_MS = 30_000

export type AmberiteAuthUser = AmberiteAccountUser

export interface UseAmberiteAuthReturn {
	user: ComputedRef<AmberiteAuthUser | null>
	isLoggedIn: ComputedRef<boolean>
	isReady: ComputedRef<boolean>
	signingIn: Ref<boolean>
	error: Ref<Error | null>
	signIn: () => Promise<void>
	logOut: () => Promise<void>
}

function mapToAuthUser(
	cu: {
		id?: string
		userId: string
		username?: string
		name?: string
		displayName?: string
		image?: string
		avatar_url?: string | null
		bio?: string | null
		createdAt?: number
	} | null,
): AmberiteAuthUser | null {
	if (!cu) return null
	return mapAmberiteUserToAccountUser({
		...cu,
		id: cu.id ?? cu.userId,
		created: cu.createdAt ? new Date(cu.createdAt).toISOString() : new Date().toISOString(),
	})
}

export function useAmberiteAuth(): UseAmberiteAuthReturn {
	const social = useSocial()
	const adapter = useCoreClient().adapter
	const authClient = new ConvexAmberiteAuthClient({ adapter })

	const user = computed<AmberiteAuthUser | null>(() => mapToAuthUser(social.currentUser.value))
	const isLoggedIn = computed(() => !!social.currentUser.value)
	const restoring = ref(true)
	const isReady = computed(() => !social.loading.value && !restoring.value)
	const signingIn = ref(false)
	const error = ref<Error | null>(null)

	async function signIn() {
		if (signingIn.value) return
		signingIn.value = true
		error.value = null
		const startedAt = Date.now()
		try {
			await withTimeout(async () => {
				const credential = await amberiteLogin()
				const devPersonaId = await amberiteDevPersonaId()
				const params: { minecraftAccessToken: string; devPersonaId?: string } = {
					minecraftAccessToken: credential.accessToken,
				}
				if (devPersonaId) params.devPersonaId = devPersonaId
				await authClient.signInWithMinecraftToken(params)
				await social.refresh()
				if (!social.currentUser.value) throw new Error('Amberite account session was not accepted.')
			}, SIGN_IN_TIMEOUT_MS)
		} catch (e) {
			console.warn('[amberite] account connection failed', e)
			const detail = e instanceof Error ? e.message : String(e)
			error.value = new Error(`Amberite could not connect your account. ${detail}`)
		} finally {
			const remaining = MIN_SIGN_IN_VISIBLE_MS - (Date.now() - startedAt)
			if (remaining > 0) {
				await new Promise((resolve) => setTimeout(resolve, remaining))
			}
			signingIn.value = false
		}
	}

	async function logOut() {
		await adapter.setCurrentJwt?.(null)
		await adapter.setCurrentRefreshToken?.(null)
		error.value = null
		await social.refresh()
	}

	void restoreSession()

	async function restoreSession() {
		try {
			const refreshToken = await adapter.getCurrentRefreshToken?.()
			if (!refreshToken) return
			const session = await authClient.refreshSession(refreshToken)
			if (!session) {
				await adapter.setCurrentJwt?.(null)
				await adapter.setCurrentRefreshToken?.(null)
				return
			}
			await social.refresh()
		} catch (e) {
			console.warn('[amberite] session refresh failed', e)
		} finally {
			restoring.value = false
		}
	}

	return { user, isLoggedIn, isReady, signingIn, error, signIn, logOut }
}

async function amberiteDevPersonaId(): Promise<string | null> {
	return await invoke<string | null>('plugin:auth|get_amberite_dev_persona_id')
}

async function withTimeout<T>(fn: () => Promise<T>, timeoutMs: number): Promise<T> {
	let timeout: ReturnType<typeof setTimeout> | undefined
	try {
		return await Promise.race([
			fn(),
			new Promise<never>((_, reject) => {
				timeout = setTimeout(
					() => reject(new Error('The Amberite connection timed out. Try again.')),
					timeoutMs,
				)
			}),
		])
	} finally {
		if (timeout) clearTimeout(timeout)
	}
}
