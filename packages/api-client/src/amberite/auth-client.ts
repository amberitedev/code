import { AmberiteApiError, AmberiteAuthError } from './errors'
import type { AmberitePlatformAdapter, PlatformMinecraftSignInRequest } from './platform'
import type { AmberiteAccountUser, AmberiteProfilePatch } from './profile'
import { normalizeAmberiteAccountUser } from './profile'
import {
	adapterSessionStorage,
	type AmberiteSessionStorage,
	refreshPlatformAmberiteSession,
	validateAmberiteSessionTokens,
} from './session'
import { ConvexAmberiteTransport, type AmberiteTransport } from './transport'

export interface AmberiteSession {
	tokens: { token: string; refreshToken: string }
	user: AmberiteAccountUser
	expiresAt?: string
}

export interface AmberiteMinecraftTokenSignInRequest {
	minecraftAccessToken: string
	expectedMinecraftUuid?: string
}

export type AmberiteMinecraftSignInRequest = PlatformMinecraftSignInRequest

export interface AmberiteDevAccountSignInRequest {
	username: string
}

export interface AmberiteAuthClientOptions {
	adapter: AmberitePlatformAdapter
	storage?: AmberiteSessionStorage
	transport?: AmberiteTransport
}

export class AmberiteAuthClient {
	private readonly adapter: AmberitePlatformAdapter
	private readonly storage: AmberiteSessionStorage
	private readonly transport: AmberiteTransport
	private refreshPromise: Promise<AmberiteSession | null> | null = null

	constructor(options: AmberiteAuthClientOptions) {
		this.adapter = options.adapter
		this.storage = options.storage ?? adapterSessionStorage(options.adapter)
		this.transport = options.transport ?? new ConvexAmberiteTransport(options.adapter)
	}

	async restoreSession(): Promise<AmberiteSession | null> {
		if (this.adapter.restoreAmberiteSession)
			return await this.restorePlatformSession(await this.adapter.restoreAmberiteSession())
		const stored = await this.storage.read()
		if (stored?.refreshToken) return await this.refreshSession(stored.refreshToken)
		const token = await this.adapter.getCurrentJwt()
		if (!token) return null
		return await this.resolveSession(token, '')
	}

	async refreshSession(refreshToken?: string | null): Promise<AmberiteSession | null> {
		if (this.refreshPromise) return await this.refreshPromise
		this.refreshPromise = this.performRefresh(refreshToken).finally(() => {
			this.refreshPromise = null
		})
		return await this.refreshPromise
	}

	async signInWithMinecraft(request: PlatformMinecraftSignInRequest): Promise<AmberiteSession> {
		if (!this.adapter.signInWithMinecraft)
			throw new AmberiteAuthError(
				'native Minecraft sign-in is not supported',
				'configuration_failure',
				'return_to_provider',
			)
		const session = await this.restorePlatformSession(
			await this.adapter.signInWithMinecraft(request),
		)
		if (!session) throw new AmberiteAuthError('Minecraft sign-in did not create a session')
		return session
	}

	async signInWithMinecraftToken(
		request: AmberiteMinecraftTokenSignInRequest,
	): Promise<AmberiteSession> {
		const response = await this.transport.action<{ tokens?: unknown }>(
			'auth:signIn',
			{
				provider: 'minecraft-token',
				params: request,
			},
			false,
		)
		if (!response.tokens) throw new AmberiteAuthError('Amberite sign-in was not accepted')
		return await this.acceptTokens(response.tokens)
	}

	async signInWithDevAccount(request: AmberiteDevAccountSignInRequest): Promise<AmberiteSession> {
		const response = await this.transport.action<{ tokens?: unknown }>(
			'auth:signIn',
			{ provider: 'amberite-dev-account', params: { username: request.username } },
			false,
		)
		if (!response.tokens) throw new AmberiteAuthError('development sign-in was not accepted')
		return await this.acceptTokens(response.tokens)
	}

	async currentUser(): Promise<AmberiteAccountUser | null> {
		try {
			return normalizeAmberiteAccountUser(await this.transport.query('profiles:current', {}))
		} catch (error) {
			if (error instanceof AmberiteAuthError && error.code === 'invalid_session') return null
			throw error
		}
	}

	async updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteAccountUser> {
		return normalizeAmberiteAccountUser(
			await this.transport.mutation('profiles:updateCurrent', patch),
		)
	}

	async deleteCurrentAccount(): Promise<void> {
		await this.transport.mutation('auth:deleteCurrentAccount', {})
		await this.adapter.signOutAmberiteSession?.().catch(() => undefined)
		await this.clearLocalSession()
	}

	async logOut(): Promise<void> {
		try {
			await this.transport.action('auth:signOut', {})
		} catch {
			// Signing out locally must work while Convex is unreachable.
		} finally {
			await this.adapter.signOutAmberiteSession?.().catch(() => undefined)
			await this.clearLocalSession()
		}
	}

	private async performRefresh(refreshToken?: string | null): Promise<AmberiteSession | null> {
		if (!refreshToken && this.adapter.refreshAmberiteSession) {
			try {
				return await this.restorePlatformSession(await refreshPlatformAmberiteSession(this.adapter))
			} catch (error) {
				if (terminalAuthError(error)) await this.clearLocalSession()
				throw error
			}
		}
		const token = refreshToken ?? (await this.storage.read())?.refreshToken
		if (!token) return null
		try {
			const response = await this.transport.action<{ tokens?: unknown }>(
				'auth:signIn',
				{ refreshToken: token },
				false,
			)
			if (!response.tokens) {
				await this.clearLocalSession()
				return null
			}
			return await this.acceptTokens(response.tokens)
		} catch (error) {
			if (terminalAuthError(error)) await this.clearLocalSession()
			throw error
		}
	}

	private async acceptTokens(value: unknown): Promise<AmberiteSession> {
		const tokens = validateAmberiteSessionTokens(value)
		await this.storage.write(tokens)
		await this.adapter.setCurrentJwt?.(tokens.token)
		return await this.resolveSession(tokens.token, tokens.refreshToken)
	}

	private async restorePlatformSession(
		session: { accessToken: string; expiresAt?: string; user?: unknown } | null,
	): Promise<AmberiteSession | null> {
		if (!session?.accessToken) return null
		await this.adapter.setCurrentJwt?.(session.accessToken)
		try {
			const user = session.user
				? normalizeAmberiteAccountUser(session.user)
				: await this.currentUser()
			if (!user) throw new AmberiteAuthError('Amberite session did not resolve a user')
			await this.adapter.setCurrentAmberiteUserId?.(user.id)
			return {
				tokens: { token: session.accessToken, refreshToken: '' },
				user,
				expiresAt: session.expiresAt,
			}
		} catch (error) {
			if (terminalAuthError(error)) await this.clearLocalSession()
			throw error
		}
	}

	private async resolveSession(token: string, refreshToken: string): Promise<AmberiteSession> {
		const user = await this.currentUser()
		if (!user) throw new AmberiteAuthError('Amberite session did not resolve a user')
		await this.adapter.setCurrentAmberiteUserId?.(user.id)
		return { tokens: { token, refreshToken }, user }
	}

	private async clearLocalSession() {
		await Promise.allSettled([
			this.storage.clear(),
			this.adapter.setCurrentJwt?.(null),
			this.adapter.setCurrentAmberiteUserId?.(null),
		])
	}
}

function terminalAuthError(error: unknown) {
	return error instanceof AmberiteApiError && error.recovery === 'clear_session'
}
