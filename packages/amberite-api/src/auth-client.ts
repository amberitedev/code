import type { PlatformAdapter } from './adapter'
import { ConvexApiClient } from './convex-api'
import type { AmberiteProfilePatch, AmberiteAccountUser } from './profile'
import { mapAmberiteUserToAccountUser } from './profile'
import {
	adapterSessionStorage,
	type AmberiteSessionStorage,
	type AmberiteSessionTokens,
	validateAmberiteSessionTokens,
} from './session'
import { AuthError } from './errors'

export interface AmberiteSession {
	tokens: AmberiteSessionTokens
	user: AmberiteAccountUser
}

export interface AmberiteMinecraftTokenSignInRequest {
	minecraftAccessToken: string
	devPersonaId?: string
}

export interface AmberiteAuthClient {
	restoreSession(): Promise<AmberiteSession | null>
	refreshSession(refreshToken?: string | null): Promise<AmberiteSession | null>
	signInWithMinecraftToken(request: AmberiteMinecraftTokenSignInRequest): Promise<AmberiteSession>
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

	constructor(options: ConvexAmberiteAuthClientOptions) {
		this.adapter = options.adapter
		this.storage = options.sessionStorage ?? adapterSessionStorage(options.adapter)
		this.convex = options.convexClient ?? new ConvexApiClient(options.adapter)
	}

	async restoreSession(): Promise<AmberiteSession | null> {
		const stored = await this.storage.read()
		if (stored?.refreshToken) {
			return await this.refreshSession(stored.refreshToken)
		}

		const token = await this.adapter.getCurrentJwt()
		if (!token) return null

		try {
			const user = await this.currentUser()
			return user ? { tokens: { token, refreshToken: '' }, user } : null
		} catch {
			await this.storage.clear()
			return null
		}
	}

	async refreshSession(refreshToken?: string | null): Promise<AmberiteSession | null> {
		const token = refreshToken ?? (await this.storage.read())?.refreshToken
		if (!token) return null

		const response = await this.convex.rawAction<{ tokens: unknown }>(
			'auth:signIn',
			{ refreshToken: token },
			false,
		)
		if (!response.tokens) {
			await this.storage.clear()
			return null
		}

		return await this.acceptTokens(response.tokens)
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
					...(request.devPersonaId ? { devPersonaId: request.devPersonaId } : {}),
				},
			},
			false,
		)
		return await this.requireTokens(response.tokens)
	}

	async currentUser(): Promise<AmberiteAccountUser | null> {
		const profile = await this.convex.currentProfile().catch((error) => {
			if (error instanceof Error && error.message.toLowerCase().includes('not authenticated'))
				return null
			throw error
		})
		return profile ? mapAmberiteUserToAccountUser(profile) : null
	}

	async updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteAccountUser> {
		const profile = await this.convex.updateCurrentProfile(patch)
		return mapAmberiteUserToAccountUser(profile)
	}

	async deleteCurrentAccount(): Promise<void> {
		await this.convex.rawMutation('auth:deleteCurrentAccount', {})
		await this.storage.clear()
	}

	async logOut(): Promise<void> {
		let ignoredLogoutError: unknown
		try {
			await this.convex.rawAction('auth:signOut', {})
		} catch (error) {
			ignoredLogoutError = error
		} finally {
			void ignoredLogoutError
			await this.storage.clear()
		}
	}

	private async requireTokens(tokens: unknown): Promise<AmberiteSession> {
		if (!tokens) throw new AuthError('Amberite sign-in was not accepted')
		return await this.acceptTokens(tokens)
	}

	private async acceptTokens(tokens: unknown): Promise<AmberiteSession> {
		const validated = validateAmberiteSessionTokens(tokens)
		await this.storage.write(validated)
		const user = await this.currentUser()
		if (!user) throw new AuthError('Amberite session did not resolve a user')
		return { tokens: validated, user }
	}
}
