import type { PlatformAdapter, PlatformMinecraftSignInRequest } from './adapter'
import { ConvexApiClient } from './convex-api'
import { AmberiteApiError, AuthError } from './errors'
import type { AmberiteAccountUser, AmberiteProfilePatch } from './profile'
import { mapAmberiteUserToAccountUser, normalizeAmberiteAccountUser } from './profile'
import {
	adapterSessionStorage,
	type AmberiteSessionStorage,
	type AmberiteSessionTokens,
	validateAmberiteSessionTokens,
} from './session'

export interface AmberiteSession {
	tokens: AmberiteSessionTokens
	user: AmberiteAccountUser
}

export interface AmberiteMinecraftTokenSignInRequest {
	minecraftAccessToken: string
	expectedMinecraftUuid?: string
}

export type AmberiteMinecraftSignInRequest = PlatformMinecraftSignInRequest

export interface AmberiteDevAccountSignInRequest {
	username: string
}

export interface AmberiteAuthClient {
	restoreSession(): Promise<AmberiteSession | null>
	refreshSession(refreshToken?: string | null): Promise<AmberiteSession | null>
	signInWithMinecraft(request: AmberiteMinecraftSignInRequest): Promise<AmberiteSession>
	signInWithMinecraftToken(request: AmberiteMinecraftTokenSignInRequest): Promise<AmberiteSession>
	signInWithDevAccount(request: AmberiteDevAccountSignInRequest): Promise<AmberiteSession>
	currentUser(): Promise<AmberiteAccountUser | null>
	updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteAccountUser>
	deleteCurrentAccount(): Promise<void>
	logOut(): Promise<void>
}

export interface ConvexAmberiteAuthClientOptions {
	adapter: PlatformAdapter
	sessionStorage?: AmberiteSessionStorage
	convexClient?: ConvexApiClient
}

export class ConvexAmberiteAuthClient implements AmberiteAuthClient {
	private readonly adapter: PlatformAdapter
	private readonly storage: AmberiteSessionStorage
	private readonly convex: ConvexApiClient
	private readonly refreshPromises = new Map<string, Promise<AmberiteSession | null>>()

	constructor(options: ConvexAmberiteAuthClientOptions) {
		this.adapter = options.adapter
		this.storage = options.sessionStorage ?? adapterSessionStorage(options.adapter)
		this.convex = options.convexClient ?? new ConvexApiClient(options.adapter)
	}

	async restoreSession(): Promise<AmberiteSession | null> {
		if (this.adapter.restoreMinecraftSession) {
			return await this.restorePlatformSession(await this.adapter.restoreMinecraftSession())
		}

		const stored = await this.storage.read()
		if (stored?.refreshToken) return await this.refreshSession(stored.refreshToken)

		const token = await this.adapter.getCurrentJwt()
		if (!token) return null
		try {
			const user = await this.currentUser()
			return user ? { tokens: { token, refreshToken: '' }, user } : null
		} catch (error) {
			if (isTerminalAuthError(error)) await this.clearLocalSession()
			throw error
		}
	}

	async refreshSession(refreshToken?: string | null): Promise<AmberiteSession | null> {
		const key = refreshToken ? `token:${refreshToken}` : 'platform-or-stored'
		const pending = this.refreshPromises.get(key)
		if (pending) return await pending
		const refresh = this.performRefresh(refreshToken).finally(() => {
			this.refreshPromises.delete(key)
		})
		this.refreshPromises.set(key, refresh)
		return await refresh
	}

	async signInWithMinecraft(request: AmberiteMinecraftSignInRequest): Promise<AmberiteSession> {
		if (!this.adapter.signInWithMinecraft) {
			throw new AuthError(
				'native Minecraft sign-in is not supported',
				'configuration_failure',
				'return_to_provider',
			)
		}
		return await this.requirePlatformSession(await this.adapter.signInWithMinecraft(request))
	}

	async signInWithMinecraftToken(
		request: AmberiteMinecraftTokenSignInRequest,
	): Promise<AmberiteSession> {
		const response = await this.convex.rawAction<{ tokens: unknown }>(
			'auth:signIn',
			{
				provider: 'minecraft-token',
				params: {
					minecraftAccessToken: request.minecraftAccessToken,
					...(request.expectedMinecraftUuid
						? { expectedMinecraftUuid: request.expectedMinecraftUuid }
						: {}),
				},
			},
			false,
		)
		return await this.requireTokens(response.tokens)
	}

	async signInWithDevAccount(request: AmberiteDevAccountSignInRequest): Promise<AmberiteSession> {
		const response = await this.convex.rawAction<{ tokens: unknown }>(
			'auth:signIn',
			{ provider: 'amberite-dev-account', params: { username: request.username } },
			false,
		)
		return await this.requireTokens(response.tokens)
	}

	async currentUser(): Promise<AmberiteAccountUser | null> {
		const profile = await this.convex.currentProfile().catch((error) => {
			if (error instanceof AuthError && error.code === 'invalid_session') return null
			throw error
		})
		return profile ? mapAmberiteUserToAccountUser(profile) : null
	}

	async updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteAccountUser> {
		return mapAmberiteUserToAccountUser(await this.convex.updateCurrentProfile(patch))
	}

	async deleteCurrentAccount(): Promise<void> {
		await this.convex.rawMutation('auth:deleteCurrentAccount', {})
		await this.clearLocalSession()
	}

	async logOut(): Promise<void> {
		try {
			await this.convex.rawAction('auth:signOut', {})
		} catch {
			// Local sign-out must complete even when remote revocation is unreachable.
		} finally {
			await this.adapter.signOutMinecraftSession?.().catch(() => undefined)
			await this.clearLocalSession()
		}
	}

	private async performRefresh(refreshToken?: string | null): Promise<AmberiteSession | null> {
		if (!refreshToken && this.adapter.refreshAmberiteSession) {
			try {
				return await this.restorePlatformSession(await this.adapter.refreshAmberiteSession())
			} catch (error) {
				if (isTerminalAuthError(error)) await this.clearLocalSession()
				throw error
			}
		}

		const token = refreshToken ?? (await this.storage.read())?.refreshToken
		if (!token) return null
		try {
			const response = await this.convex.rawAction<{ tokens: unknown }>(
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
			if (isTerminalAuthError(error)) await this.clearLocalSession()
			throw error
		}
	}

	private async requireTokens(tokens: unknown): Promise<AmberiteSession> {
		if (!tokens) throw new AuthError('Amberite sign-in was not accepted')
		return await this.acceptTokens(tokens)
	}

	private async acceptTokens(tokens: unknown): Promise<AmberiteSession> {
		const validated = validateAmberiteSessionTokens(tokens)
		const previous = await this.storage.read()
		await this.storage.write(validated)
		try {
			const user = await this.currentUser()
			if (!user) throw new AuthError('Amberite session did not resolve a user')
			return { tokens: validated, user }
		} catch (error) {
			if (previous) await this.storage.write(previous)
			else await this.storage.clear()
			throw error
		}
	}

	private async requirePlatformSession(session: {
		accessToken: string
		user?: unknown
	}): Promise<AmberiteSession> {
		const restored = await this.restorePlatformSession(session)
		if (!restored) throw new AuthError('native Minecraft sign-in did not resolve a user')
		return restored
	}

	private async restorePlatformSession(
		session: { accessToken: string; user?: unknown } | null,
	): Promise<AmberiteSession | null> {
		if (!session?.accessToken) return null
		await this.adapter.setCurrentJwt?.(session.accessToken)
		try {
			const user =
				session.user === undefined
					? await this.currentUser()
					: normalizeAmberiteAccountUser(session.user)
			if (!user) throw new AuthError('native product session did not resolve a user')
			return { tokens: { token: session.accessToken, refreshToken: '' }, user }
		} catch (error) {
			if (isTerminalAuthError(error)) {
				await this.adapter.signOutMinecraftSession?.().catch(() => undefined)
				await this.clearLocalSession()
			}
			throw error
		}
	}

	private async clearLocalSession(): Promise<void> {
		await Promise.allSettled([this.storage.clear(), this.adapter.setCurrentJwt?.(null)])
	}
}

function isTerminalAuthError(error: unknown): boolean {
	return error instanceof AmberiteApiError && error.recovery === 'clear_session'
}
