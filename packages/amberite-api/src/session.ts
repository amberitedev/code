import type { PlatformAdapter } from './adapter'
import { AuthError } from './errors'

export interface AmberiteSessionTokens {
	token: string
	refreshToken: string
}

export interface AmberiteSessionStorage {
	read(): Promise<AmberiteSessionTokens | null>
	write(tokens: AmberiteSessionTokens): Promise<void>
	clear(): Promise<void>
}

export function isAmberiteSessionTokens(value: unknown): value is AmberiteSessionTokens {
	if (!value || typeof value !== 'object') return false
	const candidate = value as Record<string, unknown>
	return typeof candidate.token === 'string' && typeof candidate.refreshToken === 'string'
}

export function validateAmberiteSessionTokens(value: unknown): AmberiteSessionTokens {
	if (!isAmberiteSessionTokens(value) || !value.token.trim() || !value.refreshToken.trim()) {
		throw new AuthError('invalid Amberite session token response', 'corrupt_session')
	}
	return { token: value.token, refreshToken: value.refreshToken }
}

export function adapterSessionStorage(adapter: PlatformAdapter): AmberiteSessionStorage {
	return {
		async read(): Promise<AmberiteSessionTokens | null> {
			if (adapter.readAmberiteSession) return await adapter.readAmberiteSession()
			const token = await adapter.getCurrentJwt()
			const refreshToken = await adapter.getCurrentRefreshToken?.()
			if (!token || !refreshToken) return null
			return { token, refreshToken }
		},

		async write(tokens: AmberiteSessionTokens): Promise<void> {
			if (adapter.writeAmberiteSession) {
				await adapter.writeAmberiteSession(tokens)
				return
			}
			if (!adapter.setCurrentJwt || !adapter.setCurrentRefreshToken) {
				throw new AuthError('Amberite session storage is not writable', 'corrupt_session')
			}
			const previous = await this.read()
			try {
				await adapter.setCurrentJwt(tokens.token)
				await adapter.setCurrentRefreshToken(tokens.refreshToken)
			} catch (error) {
				await adapter.setCurrentJwt(previous?.token ?? null).catch(() => undefined)
				await adapter.setCurrentRefreshToken(previous?.refreshToken ?? null).catch(() => undefined)
				throw error
			}
		},

		async clear(): Promise<void> {
			if (adapter.clearAmberiteSession) {
				await adapter.clearAmberiteSession()
				return
			}
			await adapter.setCurrentJwt?.(null)
			await adapter.setCurrentRefreshToken?.(null)
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
