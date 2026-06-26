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

export interface AmberitePasswordSignInRequest {
	login: string
	password: string
	challenge?: string
}

export interface AmberitePasswordSignUpRequest {
	email: string
	password: string
	username: string
	challenge?: string
}

export interface AmberiteMinecraftTokenSignInRequest {
	minecraftAccessToken: string
	devPersonaId?: string
}

export interface AmberiteAuthClient {
	restoreSession(): Promise<AmberiteSession | null>
	refreshSession(refreshToken?: string | null): Promise<AmberiteSession | null>
	signInWithPassword(request: AmberitePasswordSignInRequest): Promise<AmberiteSession>
	signUpWithPassword(request: AmberitePasswordSignUpRequest): Promise<AmberiteSession>
	signInWithMinecraftToken(request: AmberiteMinecraftTokenSignInRequest): Promise<AmberiteSession>
	signInWithModrinthToken(modrinthToken: string): Promise<AmberiteSession>
	validatePasswordAccount(request: AmberitePasswordSignUpRequest): Promise<{ ok: true }>
	currentUser(): Promise<AmberiteAccountUser | null>
	updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteAccountUser>
	updateEmail(email: string): Promise<AmberiteAccountUser>
	updatePassword(request: { oldPassword?: string | null; newPassword?: string | null }): Promise<AmberiteAccountUser>
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

	async signInWithPassword(request: AmberitePasswordSignInRequest): Promise<AmberiteSession> {
		const response = await this.convex.rawAction<{ tokens: unknown }>(
			'auth:signIn',
			{
				provider: 'web-password',
				params: {
					flow: 'signIn',
					login: request.login,
					password: request.password,
					...(request.challenge ? { challenge: request.challenge } : {}),
				},
			},
			false,
		)
		return await this.requireTokens(response.tokens)
	}

	async signUpWithPassword(request: AmberitePasswordSignUpRequest): Promise<AmberiteSession> {
		const response = await this.convex.rawAction<{ tokens: unknown }>(
			'auth:signIn',
			{
				provider: 'web-password',
				params: {
					flow: 'signUp',
					email: request.email,
					password: request.password,
					username: request.username,
					...(request.challenge ? { challenge: request.challenge } : {}),
				},
			},
			false,
		)
		return await this.requireTokens(response.tokens)
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

	async signInWithModrinthToken(modrinthToken: string): Promise<AmberiteSession> {
		const response = await this.convex.rawAction<{ tokens: unknown }>(
			'auth:signIn',
			{
				provider: 'modrinth-token',
				params: { modrinthToken },
			},
			false,
		)
		return await this.requireTokens(response.tokens)
	}

	async validatePasswordAccount(request: AmberitePasswordSignUpRequest): Promise<{ ok: true }> {
		await this.convex.rawQuery('auth:validateWebPasswordAccount', {
			email: request.email,
			password: request.password,
			username: request.username,
		})
		return { ok: true }
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

	async updateEmail(email: string): Promise<AmberiteAccountUser> {
		const profile = await this.convex.rawMutation('auth:updateCurrentEmail', { email })
		return mapAmberiteUserToAccountUser(profile as any)
	}

	async updatePassword(request: {
		oldPassword?: string | null
		newPassword?: string | null
	}): Promise<AmberiteAccountUser> {
		const profile = await this.convex.rawMutation('auth:updateCurrentPassword', request)
		return mapAmberiteUserToAccountUser(profile as any)
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
