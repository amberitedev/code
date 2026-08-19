import {
	type AmberiteAccountUser,
	AmberiteApiError,
	ConvexAmberiteAuthClient,
	mapAmberiteUserToAccountUser,
} from '@amberite/amberite-api'
import { invoke } from '@tauri-apps/api/core'
import type { ComputedRef, Ref } from 'vue'
import { computed, ref } from 'vue'

import { usePlatformAdapter } from '@/composables/useCoreClient'
import { useSocial } from '@/composables/useSocial'

export type AmberiteAuthUser = AmberiteAccountUser
export type AmberiteAuthGate =
	| 'restoring'
	| 'signedOut'
	| 'connecting'
	| 'authenticated'
	| 'offlineRetrying'
	| 'reauthRequired'
	| 'connectionError'

export type MinecraftAuthState = 'none' | 'ready' | 'reauthRequired' | 'authenticating'

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
	minecraftStatus: Ref<MinecraftAuthState>
	canUseLauncher: ComputedRef<boolean>
	canUseLocalLauncher: ComputedRef<boolean>
	canLaunch: ComputedRef<boolean>
	canUseCloud: ComputedRef<boolean>
	needsReauth: ComputedRef<boolean>
	isOffline: ComputedRef<boolean>
	hasPermanentCloudError: ComputedRef<boolean>
	status: Ref<AmberiteAuthGate>
	isLoggedIn: ComputedRef<boolean>
	isReady: ComputedRef<boolean>
	signingIn: ComputedRef<boolean>
	error: Ref<Error | null>
	serverUnavailable: Ref<boolean>
	initialize: () => Promise<void>
	signIn: (mode?: 'continue' | 'use_another_account') => Promise<void>
	retryRestore: () => Promise<void>
	logOut: () => Promise<void>
}

const social = useSocial()
const authClient = new ConvexAmberiteAuthClient({ adapter: usePlatformAdapter() })
const status = ref<AmberiteAuthGate>('restoring')
const error = ref<Error | null>(null)
const serverUnavailable = ref(false)
const rememberedIdentity = ref<RememberedAmberiteIdentity | null>(null)
const sessionUser = ref<AmberiteAuthUser | null>(null)
const hasMinecraftAccess = ref(false)
const hasVerifiedMinecraftAccess = ref(false)
const hasActiveMinecraftAccount = ref(false)
const minecraftStatus = ref<MinecraftAuthState>('none')
let restorePromise: Promise<void> | null = null
let signInPromise: Promise<void> | null = null
let refreshPromise: Promise<void> | null = null
let refreshTimer: ReturnType<typeof setTimeout> | null = null
let errorTimer: ReturnType<typeof setTimeout> | null = null
const OFFLINE_REFRESH_RETRY_MS = 5 * 60 * 1_000
const REFRESH_EARLY_MS = 2 * 60 * 1_000
const ERROR_DISMISS_MS = 2_000

const user = computed<AmberiteAuthUser | null>(() =>
	status.value === 'authenticated'
		? (sessionUser.value ?? mapSocialUser(social.currentUser.value))
		: null,
)
const isLoggedIn = computed(() => status.value === 'authenticated' && Boolean(user.value))
const isReady = computed(() => status.value !== 'restoring' && status.value !== 'connecting')
const signingIn = computed(() => status.value === 'connecting')
const canUseLocalLauncher = computed(
	() =>
		minecraftStatus.value === 'ready' &&
		hasVerifiedMinecraftAccess.value &&
		['authenticated', 'offlineRetrying', 'reauthRequired', 'connectionError'].includes(
			status.value,
		),
)
const canUseLauncher = canUseLocalLauncher
const canLaunch = computed(
	() => minecraftStatus.value === 'ready' && hasActiveMinecraftAccount.value,
)
const canUseCloud = computed(() => status.value === 'authenticated')
const needsReauth = computed(() => status.value === 'reauthRequired')
const isOffline = computed(() => canUseLocalLauncher.value && status.value !== 'authenticated')
const hasPermanentCloudError = computed(() => status.value === 'connectionError')

const coordinator: UseAmberiteAuthReturn = {
	user,
	rememberedIdentity,
	hasMinecraftAccess,
	minecraftStatus,
	canUseLauncher,
	canUseLocalLauncher,
	canLaunch,
	canUseCloud,
	needsReauth,
	isOffline,
	hasPermanentCloudError,
	status,
	isLoggedIn,
	isReady,
	signingIn,
	error,
	serverUnavailable,
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
		const session =
			devConfig?.authMode === 'dev'
				? await signInWithDevAccount(devConfig.username!)
				: await authClient.restoreSession()
		if (!session) {
			sessionUser.value = null
			status.value = hasVerifiedMinecraftAccess.value ? 'reauthRequired' : 'signedOut'
			if (status.value === 'signedOut') await checkSignedOutAvailability()
			return
		}
		sessionUser.value = session.user
		if (devConfig?.authMode === 'dev') {
			hasMinecraftAccess.value = true
			hasActiveMinecraftAccount.value = true
			hasVerifiedMinecraftAccess.value = true
			minecraftStatus.value = 'ready'
		} else {
			hasMinecraftAccess.value = true
		}
		await social.refresh().catch(() => undefined)
		status.value = 'authenticated'
		scheduleRefresh(session.expiresAt)
	} catch (value) {
		sessionUser.value = null
		showAuthError(value)
		if (shouldPreserveSession(value) && hasVerifiedMinecraftAccess.value) {
			status.value = 'offlineRetrying'
			scheduleRefresh(undefined, OFFLINE_REFRESH_RETRY_MS)
		} else {
			status.value = hasVerifiedMinecraftAccess.value ? 'reauthRequired' : 'signedOut'
		}
	}
}

async function retryRestore(): Promise<void> {
	if (status.value === 'restoring') return await restorePromise
	restorePromise = restoreSession()
	await restorePromise
}

async function refreshMinecraftAccess(): Promise<void> {
	const accounts = await invoke<Array<{ active: boolean; profile: { id: string } }>>(
		'plugin:auth|get_users',
	).catch(() => [])
	hasMinecraftAccess.value = accounts.length > 0
	hasActiveMinecraftAccount.value = accounts.some((account) => account.active)
	hasVerifiedMinecraftAccess.value = accounts.some(
		(account) =>
			account.profile.id.toLowerCase() === rememberedIdentity.value?.minecraftUuid.toLowerCase(),
	)
	minecraftStatus.value = hasMinecraftAccess.value ? 'ready' : 'none'
}

async function checkSignedOutAvailability(): Promise<void> {
	try {
		await invoke('plugin:auth|check_amberite_reachable', {
			convexUrl: usePlatformAdapter().convexUrl,
		})
		clearAuthError()
	} catch (value) {
		showAuthError(value, true)
	}
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
	status.value = 'connecting'
	minecraftStatus.value = 'authenticating'
	clearAuthError()
	try {
		const session = await authClient.signInWithMinecraft({
			mode,
			...(mode === 'continue' && rememberedIdentity.value
				? { expectedMinecraftUuid: rememberedIdentity.value.minecraftUuid }
				: {}),
		})
		sessionUser.value = session.user
		hasMinecraftAccess.value = true
		hasActiveMinecraftAccount.value = true
		minecraftStatus.value = 'ready'
		await social.refresh().catch(() => undefined)
		await loadRememberedIdentity()
		hasVerifiedMinecraftAccess.value = true
		status.value = 'authenticated'
		scheduleRefresh(session.expiresAt)
	} catch (value) {
		await refreshMinecraftAccess()
		if (isCancelledAuthError(value)) {
			clearAuthError()
			minecraftStatus.value = hasMinecraftAccess.value ? 'ready' : 'none'
			sessionUser.value = previousUser
			status.value = previousStatus
			return
		}
		showAuthError(value, true)
		if (previousStatus === 'authenticated' && previousUser) {
			sessionUser.value = previousUser
			status.value = 'authenticated'
			scheduleRefresh()
		} else if (shouldPreserveSession(value) && hasVerifiedMinecraftAccess.value) {
			sessionUser.value = null
			status.value = 'offlineRetrying'
			scheduleRefresh(undefined, OFFLINE_REFRESH_RETRY_MS)
		} else if (isIdentityMismatch(value)) {
			sessionUser.value = null
			rememberedIdentity.value = null
			hasVerifiedMinecraftAccess.value = false
			status.value = 'signedOut'
			clearScheduledRefresh()
		} else if (hasVerifiedMinecraftAccess.value) {
			sessionUser.value = null
			status.value = 'connectionError'
			clearScheduledRefresh()
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
		await loadRememberedIdentity()
		status.value = hasVerifiedMinecraftAccess.value ? 'reauthRequired' : 'signedOut'
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

function scheduleRefresh(expiresAt?: string, fallbackDelay?: number): void {
	clearScheduledRefresh()
	const expiry = expiresAt ? Date.parse(expiresAt) : Number.NaN
	const delay = Number.isFinite(expiry)
		? Math.max(5_000, expiry - Date.now() - REFRESH_EARLY_MS)
		: (fallbackDelay ?? 10 * 60 * 1_000)
	refreshTimer = setTimeout(() => {
		if (status.value === 'offlineRetrying') void retryRestore()
		else void refreshSession()
	}, delay)
}

function clearScheduledRefresh(): void {
	if (refreshTimer) clearTimeout(refreshTimer)
	refreshTimer = null
}

async function refreshSession(): Promise<void> {
	if (status.value !== 'authenticated' && status.value !== 'offlineRetrying') return
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
		scheduleRefresh(session.expiresAt)
	} catch (value) {
		showAuthError(value)
		if (shouldPreserveSession(value)) {
			// Keep the authenticated shell and native credentials while offline.
			status.value = 'offlineRetrying'
			scheduleRefresh(undefined, OFFLINE_REFRESH_RETRY_MS)
			return
		}
		sessionUser.value = null
		status.value = hasVerifiedMinecraftAccess.value ? 'reauthRequired' : 'signedOut'
		clearScheduledRefresh()
		await social.refresh().catch(() => undefined)
	}
}

function isIdentityMismatch(value: unknown): boolean {
	return value instanceof AmberiteApiError && value.code === 'identity_mismatch'
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

function isCancelledAuthError(value: unknown): boolean {
	return value instanceof AmberiteApiError && value.code === 'cancelled'
}

function showAuthError(value: unknown, persistent = false): void {
	clearAuthError()
	serverUnavailable.value = shouldPreserveSession(value)
	error.value = new Error(userFacingAuthError(value))
	if (!persistent) {
		errorTimer = setTimeout(() => {
			error.value = null
			errorTimer = null
		}, ERROR_DISMISS_MS)
	}
}

function clearAuthError(): void {
	if (errorTimer) clearTimeout(errorTimer)
	errorTimer = null
	error.value = null
	serverUnavailable.value = false
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
		case 'identity_conflict':
			return 'Your Amberite account data needs repair. Try again later.'
		case 'java_profile_missing':
			return 'A Java Edition profile is required.'
		case 'xbox_restriction':
			return 'Xbox access is restricted.'
		case 'throttled':
			return 'Too many attempts. Try again later.'
		case 'configuration_failure':
			return "This Amberite build's sign-in configuration is invalid."
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
		if (status.value === 'offlineRetrying') void retryRestore()
		else if (status.value === 'authenticated' && error.value) void refreshSession()
		else if (status.value === 'signedOut') void checkSignedOutAvailability()
	})
}
