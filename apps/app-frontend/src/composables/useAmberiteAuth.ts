import {
	type AmberiteAccountUser,
	AmberiteApiError,
	ConvexAmberiteAuthClient,
	mapAmberiteUserToAccountUser,
} from '@amberite/amberite-api'
import { invoke } from '@tauri-apps/api/core'
import type { ComputedRef, Ref } from 'vue'
import { computed, ref } from 'vue'

import { useCoreClient } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'

export type AmberiteAuthUser = AmberiteAccountUser
export type AmberiteAuthGate =
	| 'restoring'
	| 'signedOut'
	| 'verifying'
	| 'retryableOffline'
	| 'authenticated'

export interface RememberedAmberiteIdentity {
	minecraftUuid: string
	verifiedMinecraftHandle: string
	displayName: string
	avatarUrl: string | null
	lastSuccessfulSignIn: string
}

export interface UseAmberiteAuthReturn {
	user: ComputedRef<AmberiteAuthUser | null>
	rememberedIdentity: Ref<RememberedAmberiteIdentity | null>
	hasMinecraftAccess: Ref<boolean>
	canUseLauncher: ComputedRef<boolean>
	status: Ref<AmberiteAuthGate>
	isLoggedIn: ComputedRef<boolean>
	isReady: ComputedRef<boolean>
	signingIn: ComputedRef<boolean>
	error: Ref<Error | null>
	initialize: () => Promise<void>
	signIn: (mode?: 'continue' | 'use_another_account') => Promise<void>
	retryRestore: () => Promise<void>
	logOut: () => Promise<void>
}

const social = useSocial()
const authClient = new ConvexAmberiteAuthClient({ adapter: useCoreClient().adapter })
const status = ref<AmberiteAuthGate>('restoring')
const error = ref<Error | null>(null)
const rememberedIdentity = ref<RememberedAmberiteIdentity | null>(null)
const sessionUser = ref<AmberiteAuthUser | null>(null)
const hasMinecraftAccess = ref(false)
let restorePromise: Promise<void> | null = null
let signInPromise: Promise<void> | null = null
let refreshPromise: Promise<void> | null = null
let refreshTimer: ReturnType<typeof setTimeout> | null = null
let errorTimer: ReturnType<typeof setTimeout> | null = null
const REFRESH_INTERVAL_MS = 45 * 60 * 1_000
const OFFLINE_REFRESH_RETRY_MS = 5 * 60 * 1_000
const ERROR_DISMISS_MS = 2_000

const user = computed<AmberiteAuthUser | null>(() =>
	status.value === 'authenticated'
		? (sessionUser.value ?? mapSocialUser(social.currentUser.value))
		: null,
)
const isLoggedIn = computed(() => status.value === 'authenticated' && Boolean(user.value))
const isReady = computed(() => status.value !== 'restoring' && status.value !== 'verifying')
const signingIn = computed(() => status.value === 'verifying')
const canUseLauncher = computed(
	() =>
		status.value === 'authenticated' ||
		(status.value === 'retryableOffline' && hasMinecraftAccess.value),
)

const coordinator: UseAmberiteAuthReturn = {
	user,
	rememberedIdentity,
	hasMinecraftAccess,
	canUseLauncher,
	status,
	isLoggedIn,
	isReady,
	signingIn,
	error,
	initialize,
	signIn,
	retryRestore,
	logOut,
}

export function useAmberiteAuth(): UseAmberiteAuthReturn {
	return coordinator
}

async function initialize(): Promise<void> {
	if (!restorePromise) restorePromise = restoreSession()
	await restorePromise
}

async function restoreSession(): Promise<void> {
	clearScheduledRefresh()
	status.value = 'restoring'
	clearAuthError()
	await loadRememberedIdentity()
	await refreshMinecraftAccess()
	try {
		const devConfig = await getDevConfig()
		const session = devConfig
			? await signInWithDevAccount(devConfig.username)
			: await authClient.restoreSession()
		if (!session) {
			sessionUser.value = null
			status.value = 'signedOut'
			return
		}
		sessionUser.value = session.user
		if (!devConfig) hasMinecraftAccess.value = true
		await social.refresh().catch(() => undefined)
		status.value = 'authenticated'
		scheduleRefresh()
	} catch (value) {
		sessionUser.value = null
		showAuthError(value)
		status.value = shouldPreserveSession(value) ? 'retryableOffline' : 'signedOut'
	}
}

async function retryRestore(): Promise<void> {
	if (status.value === 'restoring') return await restorePromise
	restorePromise = restoreSession()
	await restorePromise
}

async function refreshMinecraftAccess(): Promise<void> {
	const accounts = await invoke<unknown[]>('plugin:auth|get_users').catch(() => [])
	hasMinecraftAccess.value = accounts.length > 0
}

async function signIn(mode: 'continue' | 'use_another_account' = 'continue'): Promise<void> {
	if (signInPromise) return await signInPromise
	signInPromise = performSignIn(mode).finally(() => {
		signInPromise = null
	})
	await signInPromise
}

async function performSignIn(mode: 'continue' | 'use_another_account'): Promise<void> {
	const previousStatus = status.value
	const previousUser = sessionUser.value
	status.value = 'verifying'
	clearAuthError()
	try {
		const devConfig = await getDevConfig()
		const session = devConfig
			? await signInWithDevAccount(devConfig.username)
			: await authClient.signInWithMinecraft({
					mode,
					...(mode === 'continue' && rememberedIdentity.value
						? { expectedMinecraftUuid: rememberedIdentity.value.minecraftUuid }
						: {}),
				})
		sessionUser.value = session.user
		if (!devConfig) hasMinecraftAccess.value = true
		await social.refresh().catch(() => undefined)
		await loadRememberedIdentity()
		status.value = 'authenticated'
		scheduleRefresh()
	} catch (value) {
		showAuthError(value)
		if (previousStatus === 'authenticated' && previousUser) {
			sessionUser.value = previousUser
			status.value = 'authenticated'
			scheduleRefresh()
		} else {
			sessionUser.value = null
			status.value = 'signedOut'
			clearScheduledRefresh()
		}
	}
}

async function logOut(): Promise<void> {
	clearScheduledRefresh()
	try {
		await authClient.logOut()
	} finally {
		sessionUser.value = null
		clearAuthError()
		status.value = 'signedOut'
		await loadRememberedIdentity()
		await social.refresh().catch(() => undefined)
	}
}

async function signInWithDevAccount(username: string) {
	const session = await authClient.signInWithDevAccount({ username })
	if (session.user.username.toLowerCase() !== username.toLowerCase()) {
		throw new Error(`Amberite development account did not resolve user ${username}.`)
	}
	return session
}

async function loadRememberedIdentity(): Promise<void> {
	rememberedIdentity.value = await invoke<RememberedAmberiteIdentity | null>(
		'plugin:auth|get_remembered_amberite_identity',
	).catch(() => null)
}

function scheduleRefresh(delay = REFRESH_INTERVAL_MS): void {
	clearScheduledRefresh()
	refreshTimer = setTimeout(() => void refreshSession(), delay)
}

function clearScheduledRefresh(): void {
	if (refreshTimer) clearTimeout(refreshTimer)
	refreshTimer = null
}

async function refreshSession(): Promise<void> {
	if (status.value !== 'authenticated') return
	if (refreshPromise) return await refreshPromise
	refreshPromise = performRefresh().finally(() => {
		refreshPromise = null
	})
	await refreshPromise
}

async function performRefresh(): Promise<void> {
	try {
		const session = await authClient.refreshSession()
		if (!session) {
			sessionUser.value = null
			status.value = 'signedOut'
			clearScheduledRefresh()
			return
		}
		sessionUser.value = session.user
		clearAuthError()
		await social.refresh().catch(() => undefined)
		scheduleRefresh()
	} catch (value) {
		showAuthError(value)
		if (shouldPreserveSession(value)) {
			// Keep the authenticated shell and native credentials while offline.
			scheduleRefresh(OFFLINE_REFRESH_RETRY_MS)
			return
		}
		sessionUser.value = null
		status.value = 'signedOut'
		clearScheduledRefresh()
		await social.refresh().catch(() => undefined)
	}
}

function mapSocialUser(
	current: {
		id?: string
		userId: string
		username?: string
		minecraftUuid?: string
		verifiedMinecraftHandle?: string
		name?: string
		displayName?: string
		image?: string
		avatar_url?: string | null
		bio?: string | null
		createdAt?: number
	} | null,
): AmberiteAuthUser | null {
	if (!current?.minecraftUuid || !current.verifiedMinecraftHandle) return null
	return mapAmberiteUserToAccountUser({
		...current,
		id: current.id ?? current.userId,
		created: current.createdAt
			? new Date(current.createdAt).toISOString()
			: new Date().toISOString(),
	})
}

function shouldPreserveSession(value: unknown): boolean {
	if (value instanceof AmberiteApiError) return value.recovery === 'preserve_and_retry'
	const message = String(value).toLowerCase()
	return (
		message.includes('network') ||
		message.includes('connect') ||
		message.includes('timeout') ||
		message.includes('offline') ||
		message.includes('unreachable')
	)
}

function showAuthError(value: unknown): void {
	clearAuthError()
	error.value = new Error(userFacingAuthError(value))
	errorTimer = setTimeout(() => {
		error.value = null
		errorTimer = null
	}, ERROR_DISMISS_MS)
}

function clearAuthError(): void {
	if (errorTimer) clearTimeout(errorTimer)
	errorTimer = null
	error.value = null
}

function userFacingAuthError(value: unknown): string {
	const code =
		value instanceof AmberiteApiError &&
		'code' in value &&
		typeof (value as AmberiteApiError & { code?: unknown }).code === 'string'
			? (value as AmberiteApiError & { code: string }).code
			: null
	switch (code) {
		case 'cancelled':
			return 'Sign-in cancelled.'
		case 'identity_mismatch':
			return "That Minecraft account doesn't match."
		case 'java_profile_missing':
			return 'A Java Edition profile is required.'
		case 'xbox_restriction':
			return 'Xbox access is restricted.'
		case 'throttled':
			return 'Too many attempts. Try again later.'
		case 'configuration_failure':
			return "Sign-in isn't set up yet."
		case 'invalid_session':
		case 'expired_session':
		case 'revoked_session':
		case 'corrupt_session':
		case 'refresh_reuse':
			return 'Your session expired. Sign in again.'
		case 'offline':
		case 'timeout':
		case 'provider_unreachable':
		case 'amberite_unreachable':
			return "Can't connect right now."
		default:
			return 'Something went wrong. Try again.'
	}
}

async function getDevConfig() {
	if (!import.meta.env.DEV) return null
	const { getDevAppConfig } = await import('@/dev/runtime')
	return getDevAppConfig()
}

if (typeof window !== 'undefined') {
	window.addEventListener('online', () => {
		if (status.value === 'retryableOffline') void retryRestore()
		else if (status.value === 'authenticated' && error.value) void refreshSession()
	})
}

if (import.meta.env.DEV) {
	void import('@/dev/runtime').then(({ registerDevAccountSwitcher }) => {
		registerDevAccountSwitcher(async ({ username }) => {
			clearScheduledRefresh()
			await authClient.logOut()
			const session = await signInWithDevAccount(username)
			sessionUser.value = session.user
			await social.refresh().catch(() => undefined)
			status.value = 'authenticated'
			clearAuthError()
			scheduleRefresh()
		})
	})
}
