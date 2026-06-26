import type {
	AmberiteAuthClient,
	AmberiteMinecraftTokenSignInRequest,
	AmberitePasswordSignInRequest,
	AmberitePasswordSignUpRequest,
	AmberiteSession,
} from './auth-client'
import type { AmberiteAccountUser, AmberiteProfilePatch } from './profile'
import type { AmberiteSessionTokens } from './session'

export interface MockAmberiteAuthClientOptions {
	user?: AmberiteAccountUser | null
	tokens?: AmberiteSessionTokens
}

export class MockAmberiteAuthClient implements AmberiteAuthClient {
	private user: AmberiteAccountUser | null
	private tokens: AmberiteSessionTokens

	constructor(options: MockAmberiteAuthClientOptions = {}) {
		this.user = options.user ?? defaultMockUser()
		this.tokens = options.tokens ?? {
			token: 'mock-amberite-token',
			refreshToken: 'mock-amberite-refresh-token',
		}
	}

	async restoreSession(): Promise<AmberiteSession | null> {
		return this.user ? { tokens: this.tokens, user: this.user } : null
	}

	async refreshSession(): Promise<AmberiteSession | null> {
		return await this.restoreSession()
	}

	async signInWithPassword(_request: AmberitePasswordSignInRequest): Promise<AmberiteSession> {
		return this.requireSession()
	}

	async signUpWithPassword(request: AmberitePasswordSignUpRequest): Promise<AmberiteSession> {
		this.user = {
			...defaultMockUser(),
			id: `mock-${request.username}`,
			userId: `mock-${request.username}`,
			username: request.username,
			name: request.username,
			email: request.email,
			has_password: true,
		}
		return this.requireSession()
	}

	async signInWithMinecraftToken(
		_request: AmberiteMinecraftTokenSignInRequest,
	): Promise<AmberiteSession> {
		return this.requireSession()
	}

	async signInWithModrinthToken(_modrinthToken: string): Promise<AmberiteSession> {
		return this.requireSession()
	}

	async validatePasswordAccount(_request: AmberitePasswordSignUpRequest): Promise<{ ok: true }> {
		return { ok: true }
	}

	async currentUser(): Promise<AmberiteAccountUser | null> {
		return this.user
	}

	async updateCurrentProfile(patch: AmberiteProfilePatch): Promise<AmberiteAccountUser> {
		const current = this.requireUser()
		this.user = {
			...current,
			...(patch.username !== undefined
				? { username: patch.username, name: patch.username }
				: {}),
			...(patch.bio !== undefined ? { bio: patch.bio } : {}),
			...(patch.avatar !== undefined
				? { avatar_url: patch.avatar === null ? null : patch.avatar.url }
				: {}),
		}
		return this.user
	}

	async updateEmail(email: string): Promise<AmberiteAccountUser> {
		const current = this.requireUser()
		this.user = { ...current, email, email_verified: true }
		return this.user
	}

	async updatePassword(_request: {
		oldPassword?: string | null
		newPassword?: string | null
	}): Promise<AmberiteAccountUser> {
		const current = this.requireUser()
		this.user = { ...current, has_password: _request.newPassword !== null }
		return this.user
	}

	async deleteCurrentAccount(): Promise<void> {
		this.user = null
	}

	async logOut(): Promise<void> {
		this.user = null
	}

	private requireSession(): AmberiteSession {
		const user = this.requireUser()
		return { tokens: this.tokens, user }
	}

	private requireUser(): AmberiteAccountUser {
		if (!this.user) throw new Error('mock Amberite user is signed out')
		return this.user
	}
}

export function defaultMockUser(): AmberiteAccountUser {
	return {
		id: 'mock-user-id',
		userId: 'mock-user-id',
		username: 'devuser',
		name: 'Dev User',
		email: 'dev@example.com',
		avatar_url: null,
		bio: null,
		created: '2024-01-01T00:00:00.000Z',
		role: 'developer',
		badges: 0,
		auth_providers: [],
		email_verified: true,
		has_password: true,
		has_totp: false,
	}
}
