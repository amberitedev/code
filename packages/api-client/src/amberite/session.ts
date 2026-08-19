import { AmberiteAuthError } from './errors'
import type {
	AmberitePlatformAdapter,
	AmberiteSessionTokens,
	PlatformAuthSession,
} from './platform'

export interface AmberiteSessionStorage {
	read(): Promise<AmberiteSessionTokens | null>
	write(tokens: AmberiteSessionTokens): Promise<void>
	clear(): Promise<void>
}

const refreshes = new WeakMap<AmberitePlatformAdapter, Promise<PlatformAuthSession | null>>()

export async function refreshPlatformAmberiteSession(
	adapter: AmberitePlatformAdapter,
): Promise<PlatformAuthSession | null> {
	if (!adapter.refreshAmberiteSession) return null
	const pending = refreshes.get(adapter)
	if (pending) return await pending
	const refresh = adapter.refreshAmberiteSession().finally(() => refreshes.delete(adapter))
	refreshes.set(adapter, refresh)
	return await refresh
}

export function validateAmberiteSessionTokens(value: unknown): AmberiteSessionTokens {
	if (!isAmberiteSessionTokens(value) || !value.token.trim() || !value.refreshToken.trim())
		throw new AmberiteAuthError('invalid Amberite session token response', 'corrupt_session')
	return { token: value.token, refreshToken: value.refreshToken }
}

export function isAmberiteSessionTokens(value: unknown): value is AmberiteSessionTokens {
	if (!value || typeof value !== 'object') return false
	const tokens = value as Record<string, unknown>
	return typeof tokens.token === 'string' && typeof tokens.refreshToken === 'string'
}

export function adapterSessionStorage(adapter: AmberitePlatformAdapter): AmberiteSessionStorage {
	const atomic = adapter.amberiteSessionStorage
	return {
		async read() {
			if (atomic) return await atomic.read()
			const token = await adapter.getCurrentJwt()
			const refreshToken = await adapter.getCurrentRefreshToken?.()
			return token && refreshToken ? { token, refreshToken } : null
		},
		async write(tokens) {
			if (atomic) return await atomic.write(tokens)
			if (!adapter.setCurrentJwt || !adapter.setCurrentRefreshToken)
				throw new AmberiteAuthError('Amberite session storage is not writable', 'corrupt_session')
			await adapter.setCurrentJwt(tokens.token)
			await adapter.setCurrentRefreshToken(tokens.refreshToken)
		},
		async clear() {
			if (atomic) return await atomic.clear()
			await Promise.allSettled([
				adapter.setCurrentJwt?.(null),
				adapter.setCurrentRefreshToken?.(null),
			])
		},
	}
}

export function createMemoryAmberiteSessionStorage(
	initialTokens: AmberiteSessionTokens | null = null,
): AmberiteSessionStorage & { current(): AmberiteSessionTokens | null } {
	let currentTokens = initialTokens
	return {
		async read() {
			return currentTokens
		},
		async write(tokens) {
			currentTokens = tokens
		},
		async clear() {
			currentTokens = null
		},
		current() {
			return currentTokens
		},
	}
}
